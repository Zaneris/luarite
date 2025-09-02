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
        // Debug library is not exposed in the sandbox to reduce attack surface

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

        // Inject math.random shim to use the deterministic engine RNG.
        let math_random_shim = r#"
  do
    local r = engine.random
    function math.random(a,b)
      if a == nil then return r()
      elseif b == nil then return math.floor(r()*a) + 1
      else return math.floor(r()*(b-a+1)) + a end
    end
  end
"#;
        self.lua
            .load(math_random_shim)
            .set_name("math.random shim").map_err(|e| anyhow::anyhow!(e.to_string()))?
            .set_environment(env.clone()).map_err(|e| anyhow::anyhow!(e.to_string()))?
            .exec().map_err(|e| anyhow::anyhow!(e.to_string()))?;

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

        // Inject math.random shim to use the deterministic engine RNG.
        let math_random_shim = r#"
  do
    local r = engine.random
    function math.random(a,b)
      if a == nil then return r()
      elseif b == nil then return math.floor(r()*a) + 1
      else return math.floor(r()*(b-a+1)) + a end
    end
  end
"#;
        self.lua
            .load(math_random_shim)
            .set_name("math.random shim").map_err(|e| anyhow::anyhow!(e.to_string()))?
            .set_environment(env.clone()).map_err(|e| anyhow::anyhow!(e.to_string()))?
            .exec().map_err(|e| anyhow::anyhow!(e.to_string()))?;

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
