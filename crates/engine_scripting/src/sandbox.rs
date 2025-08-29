use anyhow::Result;
use mlua::{Function, Lua, Table, Value};

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
        let safe: Table = match self.lua.named_registry_value("safe_base") {
            Ok(t) => t,
            Err(e) => return Err(anyhow::Error::msg(format!("get safe_base failed: {}", e))),
        };
        // Inject engine into environment if present globally
        if let Ok(engine_tbl) = self.lua.globals().get::<Table>("engine") {
            if let Err(e) = safe.set("engine", engine_tbl) {
                return Err(anyhow::Error::msg(format!("safe.set engine failed: {}", e)));
            }
        }
        let chunk = self.lua.load(script_content).set_name(script_name);
        let chunk = chunk.set_environment(safe);
        match chunk.exec() {
            Ok(()) => Ok(()),
            Err(e) => Err(anyhow::Error::msg(format!("Failed to load script {}: {}", script_name, e))),
        }
    }

    pub fn call_function<A, R>(&self, func_name: &str, args: A) -> Result<R>
    where
        A: mlua::IntoLuaMulti,
        R: mlua::FromLuaMulti,
    {
        let globals = self.lua.globals();
        let func: Function = match globals.get(func_name) {
            Ok(f) => f,
            Err(e) => {
                return Err(anyhow::Error::msg(format!(
                    "Function '{}' not found: {}",
                    func_name, e
                )))
            }
        };

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
