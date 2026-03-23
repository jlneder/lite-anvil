use mlua::prelude::*;

/// Helper: get the metatable of a Lua table via `getmetatable()`.
fn lua_getmetatable(lua: &Lua, t: &LuaTable) -> LuaResult<Option<LuaTable>> {
    let getmt: LuaFunction = lua.globals().get("getmetatable")?;
    let result: LuaValue = getmt.call(t.clone())?;
    match result {
        LuaValue::Table(t) => Ok(Some(t)),
        _ => Ok(None),
    }
}

/// Registers `core.object` — the base OOP system (extend, new, is, extends).
pub fn register_preload(lua: &Lua) -> LuaResult<()> {
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "core.object",
        lua.create_function(|lua, ()| {
            let object = lua.create_table()?;

            // Object.__index = Object
            object.set("__index", object.clone())?;

            // Object:new() — default no-op constructor
            object.set("new", lua.create_function(|_lua, _self: LuaTable| Ok(()))?)?;

            // Object:extend() — create a child class
            object.set(
                "extend",
                lua.create_function(|lua, self_tbl: LuaTable| {
                    let cls = lua.create_table()?;
                    for pair in self_tbl.pairs::<LuaString, LuaValue>() {
                        let (k, v) = pair?;
                        let k_str = k.to_str()?;
                        if k_str.starts_with("__") {
                            cls.set(k, v)?;
                        }
                    }
                    cls.set("__index", cls.clone())?;
                    cls.set("super", self_tbl.clone())?;
                    let setmt: LuaFunction = lua.globals().get("setmetatable")?;
                    setmt.call::<LuaTable>((cls.clone(), self_tbl))?;
                    Ok(cls)
                })?,
            )?;

            // Object:is(T) — strict type check
            object.set(
                "is",
                lua.create_function(|lua, (self_val, t): (LuaValue, LuaValue)| {
                    let self_tbl = match self_val {
                        LuaValue::Table(t) => t,
                        _ => return Ok(false),
                    };
                    let mt = lua_getmetatable(lua, &self_tbl)?;
                    match (mt, t) {
                        (Some(mt), LuaValue::Table(t)) => Ok(mt == t),
                        _ => Ok(false),
                    }
                })?,
            )?;

            // Object:is_class_of(T)
            object.set(
                "is_class_of",
                lua.create_function(|lua, (self_val, t): (LuaValue, LuaValue)| {
                    let self_tbl = match self_val {
                        LuaValue::Table(t) => t,
                        _ => return Ok(false),
                    };
                    match t {
                        LuaValue::Table(t) => match lua_getmetatable(lua, &t)? {
                            Some(mt) => Ok(mt == self_tbl),
                            None => Ok(false),
                        },
                        _ => Ok(false),
                    }
                })?,
            )?;

            // Object:extends(T) — walk metatable chain
            object.set(
                "extends",
                lua.create_function(|lua, (self_val, t): (LuaValue, LuaValue)| {
                    let self_tbl = match self_val {
                        LuaValue::Table(t) => t,
                        _ => return Ok(false),
                    };
                    let t = match t {
                        LuaValue::Table(t) => t,
                        _ => return Ok(false),
                    };
                    let mut mt = lua_getmetatable(lua, &self_tbl)?;
                    while let Some(current) = mt {
                        if current == t {
                            return Ok(true);
                        }
                        mt = lua_getmetatable(lua, &current)?;
                    }
                    Ok(false)
                })?,
            )?;

            // Object:is_extended_by(T)
            object.set(
                "is_extended_by",
                lua.create_function(|lua, (self_val, t): (LuaValue, LuaValue)| {
                    let self_tbl = match self_val {
                        LuaValue::Table(t) => t,
                        _ => return Ok(false),
                    };
                    let t = match t {
                        LuaValue::Table(t) => t,
                        _ => return Ok(false),
                    };
                    let mut mt = lua_getmetatable(lua, &t)?;
                    while let Some(current) = mt {
                        if current == self_tbl {
                            return Ok(true);
                        }
                        let next = lua_getmetatable(lua, &current)?;
                        if next.as_ref() == Some(&current) {
                            break;
                        }
                        mt = next;
                    }
                    Ok(false)
                })?,
            )?;

            // Object:__tostring()
            object.set(
                "__tostring",
                lua.create_function(|_lua, _self: LuaTable| Ok("Object"))?,
            )?;

            // Object:__call(...) — constructor
            object.set(
                "__call",
                lua.create_function(|lua, (self_tbl, args): (LuaTable, LuaMultiValue)| {
                    let setmt: LuaFunction = lua.globals().get("setmetatable")?;
                    let obj = lua.create_table()?;
                    setmt.call::<LuaTable>((obj.clone(), self_tbl))?;
                    let new_fn: LuaValue = obj.get("new")?;
                    if let LuaValue::Function(f) = new_fn {
                        let mut call_args = vec![LuaValue::Table(obj.clone())];
                        call_args.extend(args);
                        f.call::<()>(LuaMultiValue::from_vec(call_args))?;
                    }
                    Ok(obj)
                })?,
            )?;

            Ok(LuaValue::Table(object))
        })?,
    )
}
