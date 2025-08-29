use anyhow::Result;
use mlua::{Function, Lua, Table, Value, Variadic};

pub struct LuaSandbox {
    lua: Lua,
}

impl LuaSandbox {
    pub fn new() -> Result<Self> {
        let lua = Lua::new();
        let sandbox = Self { lua };
        sandbox.setup_safe_environment()?;
        Ok(sandbox)
    }

    fn setup_safe_environment(&self) -> Result<()> {
        let globals = self.lua.globals();

        // Build a whitelisted environment (safe_base)
        let safe = match self.lua.create_table() {
            Ok(t) => t,
            Err(e) => return Err(anyhow::Error::msg(format!("create_table failed: {}", e))),
        };
        for name in [
            "assert", "pairs", "ipairs", "next", "tonumber", "tostring", "type",
        ] {
            if let Ok(v) = globals.get::<Value>(name) {
                if let Err(e) = safe.set(name, v) {
                    return Err(anyhow::Error::msg(format!("safe.set {} failed: {}", name, e)));
                }
            }
        }
        for lib in ["math", "table", "utf8"] {
            if let Ok(v) = globals.get::<Value>(lib) {
                if let Err(e) = safe.set(lib, v) {
                    return Err(anyhow::Error::msg(format!("safe.set {} failed: {}", lib, e)));
                }
            }
        }
        if let Ok(string_tbl) = globals.get::<Table>("string") {
            if let Err(e) = safe.set("string", string_tbl) {
                return Err(anyhow::Error::msg(format!("safe.set string failed: {}", e)));
            }
        }
        // Limited debug.traceback
        if let Ok(debug_tbl) = globals.get::<Table>("debug") {
            if let Ok(tb) = debug_tbl.get::<Function>("traceback") {
                let dbg = match self.lua.create_table() {
                    Ok(t) => t,
                    Err(e) => return Err(anyhow::Error::msg(format!("create_table failed: {}", e))),
                };
                if let Err(e) = dbg.set("traceback", tb) {
                    return Err(anyhow::Error::msg(format!("dbg.set traceback failed: {}", e)));
                }
                if let Err(e) = safe.set("debug", dbg) {
                    return Err(anyhow::Error::msg(format!("safe.set debug failed: {}", e)));
                }
            }
        }

        // Lock package system on globals (affects any accidental access)
        self.lock_package_system(&globals)?;

        // Store safe_base for use when loading scripts
        if let Err(e) = self.lua.set_named_registry_value("safe_base", safe) {
            return Err(anyhow::Error::msg(format!("set_named_registry_value failed: {}", e)));
        }
        Ok(())
    }

    fn lock_package_system(&self, globals: &Table) -> Result<()> {
        // Create empty package table to prevent access
        let package_table = match self.lua.create_table() {
            Ok(table) => table,
            Err(e) => {
                return Err(anyhow::Error::msg(format!(
                    "Failed to create package table: {}",
                    e
                )))
            }
        };

        if let Err(e) = package_table.set("path", "") {
            return Err(anyhow::Error::msg(format!(
                "Failed to set package.path: {}",
                e
            )));
        }

        if let Err(e) = package_table.set("cpath", "") {
            return Err(anyhow::Error::msg(format!(
                "Failed to set package.cpath: {}",
                e
            )));
        }

        if let Err(e) = globals.set("package", package_table) {
            return Err(anyhow::Error::msg(format!(
                "Failed to set package table: {}",
                e
            )));
        }

        // Replace require with our controlled version
        let controlled_require =
            match self
                .lua
                .create_function(|_lua, module_name: String| -> mlua::Result<()> {
                    Err(mlua::Error::RuntimeError(format!(
                        "Module loading disabled in sandbox: {}",
                        module_name
                    )))
                }) {
                Ok(func) => func,
                Err(e) => {
                    return Err(anyhow::Error::msg(format!(
                        "Failed to create require function: {}",
                        e
                    )))
                }
            };

        if let Err(e) = globals.set("require", controlled_require) {
            return Err(anyhow::Error::msg(format!(
                "Failed to set require function: {}",
                e
            )));
        }

        Ok(())
    }

    pub fn load_script(&self, script_content: &str, script_name: &str) -> Result<()> {
        let safe_base: Table = self
            .lua
            .named_registry_value("safe_base")
            .map_err(|e| anyhow::Error::msg(format!("get safe_base failed: {}", e)))?;

        // Create a fresh environment whose __index points to safe_base
        let env = self
            .lua
            .create_table()
            .map_err(|e| anyhow::Error::msg(format!("create_table failed: {}", e)))?;
        let mt = self
            .lua
            .create_table()
            .map_err(|e| anyhow::Error::msg(format!("create_table failed: {}", e)))?;
        mt.set("__index", safe_base)
            .map_err(|e| anyhow::Error::msg(format!("set __index failed: {}", e)))?;
        env.set_metatable(Some(mt));

        // Inject engine into env if present
        if let Ok(engine_tbl) = self.lua.globals().get::<Table>("engine") {
            env.set("engine", engine_tbl)
                .map_err(|e| anyhow::Error::msg(format!("env.set engine failed: {}", e)))?;
        }

        // Override math.random to route via engine.random
        if let Ok(engine_tbl) = env.get::<Table>("engine") {
            let engine_tbl_for_rand = engine_tbl.clone();
            let rand_func = self
                .lua
                .create_function(move |_, args: Variadic<f64>| {
                    let f: Function = engine_tbl_for_rand.get("random")?;
                    let r: f64 = f.call::<f64>(())?; // [0,1)
                    match args.len() {
                        0 => Ok(r),
                        1 => {
                            let n = args[0].floor() as i64;
                            if n <= 0 { return Err(mlua::Error::RuntimeError("math.random(n): n must be > 0".into())); }
                            Ok(((r * n as f64).floor() as i64 + 1) as f64)
                        }
                        2 => {
                            let a = args[0].floor() as i64;
                            let b = args[1].floor() as i64;
                            if b < a { return Err(mlua::Error::RuntimeError("math.random(m,n): n must be >= m".into())); }
                            let range = (b - a + 1) as f64;
                            Ok((a + (r * range).floor() as i64) as f64)
                        }
                        _ => Err(mlua::Error::RuntimeError("math.random: too many arguments".into())),
                    }
                })
                .map_err(|e| anyhow::Error::msg(format!("create math.random failed: {}", e)))?;

            let safe_base: Table = self
                .lua
                .named_registry_value("safe_base")
                .map_err(|e| anyhow::Error::msg(format!("get safe_base failed: {}", e)))?;
            if let Ok(safe_math) = safe_base.get::<Table>("math") {
                let math_override = self.lua.create_table().map_err(|e| anyhow::Error::msg(format!("create_table failed: {}", e)))?;
                let mtm = self.lua.create_table().map_err(|e| anyhow::Error::msg(format!("create_table failed: {}", e)))?;
                mtm.set("__index", safe_math).map_err(|e| anyhow::Error::msg(format!("set __index failed: {}", e)))?;
                math_override.set_metatable(Some(mtm));
                math_override.set("random", rand_func).map_err(|e| anyhow::Error::msg(format!("set math.random failed: {}", e)))?;
                env.set("math", math_override).map_err(|e| anyhow::Error::msg(format!("env.set math failed: {}", e)))?;
            }
        }

        // Load and run chunk in this environment
        let chunk = self.lua.load(script_content).set_name(script_name);
        let chunk = chunk.set_environment(env.clone());
        chunk
            .exec()
            .map_err(|e| anyhow::Error::msg(format!("Failed to load script {}: {}", script_name, e)))?;

        // Remember current env for function lookups
        self.lua
            .set_named_registry_value("current_env", env)
            .map_err(|e| anyhow::Error::msg(format!("set current_env failed: {}", e)))?;
        Ok(())
    }

    pub fn reload_script(&self, script_content: &str, script_name: &str) -> Result<()> {
        // Capture old env
        let old_env: Table = self
            .lua
            .named_registry_value("current_env")
            .map_err(|e| anyhow::Error::msg(format!("get current_env failed: {}", e)))?;

        // Build new env from safe_base
        let safe_base: Table = self
            .lua
            .named_registry_value("safe_base")
            .map_err(|e| anyhow::Error::msg(format!("get safe_base failed: {}", e)))?;
        let env = self
            .lua
            .create_table()
            .map_err(|e| anyhow::Error::msg(format!("create_table failed: {}", e)))?;
        let mt = self
            .lua
            .create_table()
            .map_err(|e| anyhow::Error::msg(format!("create_table failed: {}", e)))?;
        mt.set("__index", safe_base)
            .map_err(|e| anyhow::Error::msg(format!("set __index failed: {}", e)))?;
        env.set_metatable(Some(mt));
        if let Ok(engine_tbl) = self.lua.globals().get::<Table>("engine") {
            env.set("engine", engine_tbl)
                .map_err(|e| anyhow::Error::msg(format!("env.set engine failed: {}", e)))?;
        }

        // Override math.random on reload as well
        if let Ok(engine_tbl) = env.get::<Table>("engine") {
            let engine_tbl_for_rand = engine_tbl.clone();
            let rand_func = self
                .lua
                .create_function(move |_, args: Variadic<f64>| {
                    let f: Function = engine_tbl_for_rand.get("random")?;
                    let r: f64 = f.call::<f64>(())?;
                    match args.len() {
                        0 => Ok(r),
                        1 => {
                            let n = args[0].floor() as i64;
                            if n <= 0 { return Err(mlua::Error::RuntimeError("math.random(n): n must be > 0".into())); }
                            Ok(((r * n as f64).floor() as i64 + 1) as f64)
                        }
                        2 => {
                            let a = args[0].floor() as i64;
                            let b = args[1].floor() as i64;
                            if b < a { return Err(mlua::Error::RuntimeError("math.random(m,n): n must be >= m".into())); }
                            let range = (b - a + 1) as f64;
                            Ok((a + (r * range).floor() as i64) as f64)
                        }
                        _ => Err(mlua::Error::RuntimeError("math.random: too many arguments".into())),
                    }
                })
                .map_err(|e| anyhow::Error::msg(format!("create math.random failed: {}", e)))?;

            let safe_base: Table = self
                .lua
                .named_registry_value("safe_base")
                .map_err(|e| anyhow::Error::msg(format!("get safe_base failed: {}", e)))?;
            if let Ok(safe_math) = safe_base.get::<Table>("math") {
                let math_override = self.lua.create_table().map_err(|e| anyhow::Error::msg(format!("create_table failed: {}", e)))?;
                let mtm = self.lua.create_table().map_err(|e| anyhow::Error::msg(format!("create_table failed: {}", e)))?;
                mtm.set("__index", safe_math).map_err(|e| anyhow::Error::msg(format!("set __index failed: {}", e)))?;
                math_override.set_metatable(Some(mtm));
                math_override.set("random", rand_func).map_err(|e| anyhow::Error::msg(format!("set math.random failed: {}", e)))?;
                env.set("math", math_override).map_err(|e| anyhow::Error::msg(format!("env.set math failed: {}", e)))?;
            }
        }

        // Load script into new env
        let chunk = self.lua.load(script_content).set_name(script_name);
        let chunk = chunk.set_environment(env.clone());
        chunk
            .exec()
            .map_err(|e| anyhow::Error::msg(format!("Failed to load script {}: {}", script_name, e)))?;

        // Call new env's on_start() first to (re)initialize arrays/tables
        if let Ok(on_start) = env.get::<mlua::Function>("on_start") {
            let _ = on_start.call::<()>(());
        }
        // Then allow state migration via on_reload(old_env)
        if let Ok(on_reload) = env.get::<mlua::Function>("on_reload") {
            let _ = on_reload.call::<()>(old_env);
        }

        // Swap current env
        self.lua
            .set_named_registry_value("current_env", env)
            .map_err(|e| anyhow::Error::msg(format!("set current_env failed: {}", e)))?;

        Ok(())
    }

    pub fn call_function<A, R>(&self, func_name: &str, args: A) -> Result<R>
    where
        A: mlua::IntoLuaMulti,
        R: mlua::FromLuaMulti,
    {
        // Look up function in the current environment first
        let env: Table = self
            .lua
            .named_registry_value("current_env")
            .map_err(|e| anyhow::anyhow!("get current_env failed: {}", e))?;
        let func: Function = env
            .get(func_name)
            .map_err(|e| anyhow::anyhow!("Function '{}' not found: {}", func_name, e))?;

        match func.call(args) {
            Ok(result) => Ok(result),
            Err(e) => Err(anyhow::Error::msg(format!(
                "Error calling function '{}': {}",
                func_name, e
            ))),
        }
    }

    pub fn set_global<V>(&self, name: &str, value: V) -> Result<()>
    where
        V: mlua::IntoLua,
    {
        let globals = self.lua.globals();
        match globals.set(name, value) {
            Ok(()) => Ok(()),
            Err(e) => Err(anyhow::Error::msg(format!(
                "Failed to set global '{}': {}",
                name, e
            ))),
        }
    }

    pub fn get_global<R>(&self, name: &str) -> Result<R>
    where
        R: mlua::FromLua,
    {
        let globals = self.lua.globals();
        match globals.get(name) {
            Ok(value) => Ok(value),
            Err(e) => Err(anyhow::Error::msg(format!(
                "Failed to get global '{}': {}",
                name, e
            ))),
        }
    }

    pub fn lua(&self) -> &Lua {
        &self.lua
    }

    pub fn get_memory_usage(&self) -> f64 {
        // Get Lua memory usage in MB
        self.lua.used_memory() as f64 / 1024.0 / 1024.0
    }
}
