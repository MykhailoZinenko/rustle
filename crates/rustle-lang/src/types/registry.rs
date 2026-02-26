//! Type descriptor registry — single source of truth for built-in value types.
//!
//! Consumed by:
//!   • The resolver  — field_type / method_signature (compile-time)
//!   • The interpreter — get_field / set_field / call_method (runtime)
//!
//! Adding a new built-in type = registering one TypeDesc here.
//! No edits to interpreter.rs or resolver/ needed.

use std::collections::HashMap;

use crate::syntax::ast::Type;
use crate::error::RuntimeError;
use crate::runtime::value::Value;

// ─── Function pointer aliases ─────────────────────────────────────────────────

/// Read a field from a value. Caller guarantees `v` is the right variant.
pub type FieldGetter = fn(&Value) -> Value;

/// Return a new value with the named field replaced. `obj` is consumed.
pub type FieldSetter = fn(Value, Value) -> Value;

/// Call a method on a receiver with pre-evaluated args.
pub type MethodFn = fn(&Value, &[Value], usize) -> Result<Value, RuntimeError>;

// ─── Descriptors ──────────────────────────────────────────────────────────────

pub struct FieldDesc {
    pub name: &'static str,
    /// Type exposed to the resolver.
    pub ty:   Type,
    /// Runtime getter.
    pub get:  FieldGetter,
    /// Runtime setter — None means the field is read-only.
    pub set:  Option<FieldSetter>,
}

pub struct MethodDesc {
    pub name:   &'static str,
    /// Parameter types exposed to the resolver.
    pub params: Vec<Type>,
    /// Return type — None means void.
    pub ret:    Option<Type>,
    /// Runtime implementation. Receives pre-evaluated args.
    pub call:   MethodFn,
}

pub struct TypeDesc {
    pub name:    &'static str,
    pub fields:  Vec<FieldDesc>,
    pub methods: Vec<MethodDesc>,
}

// ─── Registry ─────────────────────────────────────────────────────────────────

pub struct TypeRegistry {
    types: HashMap<&'static str, TypeDesc>,
}

impl TypeRegistry {
    pub fn new() -> Self {
        Self { types: HashMap::new() }
    }

    pub fn register(&mut self, desc: TypeDesc) {
        self.types.insert(desc.name, desc);
    }

    // ── Resolver API — by name (used internally) ──────────────────────────────

    /// Return the type of `field` on a concrete type named `type_name`.
    pub fn field_type(&self, type_name: &str, field: &str) -> Option<Type> {
        self.types.get(type_name)?
            .fields.iter()
            .find(|f| f.name == field)
            .map(|f| f.ty.clone())
    }

    /// Return (param_types, return_type) for `method` on a concrete type named `type_name`.
    pub fn method_signature(&self, type_name: &str, method: &str)
        -> Option<(Vec<Type>, Option<Type>)>
    {
        self.types.get(type_name)?
            .methods.iter()
            .find(|m| m.name == method)
            .map(|m| (m.params.clone(), m.ret.clone()))
    }

    // ── Resolver API — by full Type (handles generics) ────────────────────────

    /// Resolve the type of `field` on any `Type`, including generic types like
    /// `res<T>` (where `.value` returns `T`) and `list<T>` (where `.len` returns float).
    pub fn resolve_field_type(&self, ty: &Type, field: &str) -> Option<Type> {
        match ty {
            // res<T>: .value returns T — the static descriptor only has a Float placeholder.
            Type::Res(inner) => match field {
                "ok"    => Some(Type::Bool),
                "value" => Some(*inner.clone()),
                "error" => Some(Type::Named("string".into())),
                _ => None,
            },
            // Named types and primitives — delegate to the static descriptor table.
            Type::Named(n) => self.field_type(n.as_str(), field),
            Type::Float    => self.field_type("float", field),
            Type::Bool     => self.field_type("bool", field),
            // list<T> only has .len; everything else returns None.
            Type::List(_)  => self.field_type("list", field),
            _ => None,
        }
    }

    /// Resolve (param_types, return_type) for `method` on any `Type`.
    /// For generic types like `list<T>`, pop() correctly returns `T`.
    pub fn resolve_method_signature(&self, ty: &Type, method: &str)
        -> Option<(Vec<Type>, Option<Type>)>
    {
        match ty {
            // list<T>: pop() returns T, push(T) accepts T — dynamic, not in static table.
            Type::List(elem) => match method {
                "push" => Some((vec![*elem.clone()], None)),
                "pop"  => Some((vec![], Some(*elem.clone()))),
                "len"  => Some((vec![], Some(Type::Float))),
                _ => None,
            },
            // array<T, N>: same idea.
            Type::Array(elem, _) => match method {
                "len" => Some((vec![], Some(Type::Float))),
                "pop" => Some((vec![], Some(*elem.clone()))),
                _ => None,
            },
            // Named types and primitives — delegate to static descriptor table.
            Type::Named(n) => self.method_signature(n.as_str(), method),
            Type::Float    => self.method_signature("float", method),
            Type::Bool     => self.method_signature("bool", method),
            _ => None,
        }
    }

    // ── Interpreter API ───────────────────────────────────────────────────────

    /// Get the value of `field` from `v`.
    /// Returns None if the type or field isn't registered.
    pub fn get_field(&self, v: &Value, field: &str) -> Option<Value> {
        let key = value_type_key(v);
        self.types.get(key)?
            .fields.iter()
            .find(|f| f.name == field)
            .map(|f| (f.get)(v))
    }

    /// Return a new Value with `field` set to `new_val`.
    /// Returns None if the type/field isn't registered or the field is read-only.
    pub fn set_field(&self, v: Value, field: &str, new_val: Value) -> Option<Value> {
        let key = value_type_key(&v);
        let setter = self.types.get(key)?
            .fields.iter()
            .find(|f| f.name == field)?
            .set?;
        Some(setter(v, new_val))
    }

    /// Call `method` on `recv` with pre-evaluated `args`.
    /// Returns None if the type or method isn't registered.
    pub fn call_method(
        &self,
        recv:   &Value,
        method: &str,
        args:   &[Value],
        line:   usize,
    ) -> Option<Result<Value, RuntimeError>> {
        let key = value_type_key(recv);
        self.types.get(key)?
            .methods.iter()
            .find(|m| m.name == method)
            .map(|m| {
                if args.len() != m.params.len() {
                    return Err(RuntimeError::new(line, format!(
                        "`{}` expects {} argument(s), got {}",
                        method, m.params.len(), args.len()
                    )));
                }
                (m.call)(recv, args, line)
            })
    }
}

impl Default for TypeRegistry {
    fn default() -> Self {
        let mut r = Self::new();
        // Primitives — empty now, methods added here as language grows
        r.register(float_desc());
        r.register(bool_desc());
        r.register(string_desc());
        // Geometric types
        r.register(vec2_desc());
        r.register(vec3_desc());
        r.register(vec4_desc());
        r.register(color_desc());
        r.register(mat3_desc());
        r.register(mat4_desc());
        // Domain types
        r.register(transform_desc());
        r.register(shape_desc());
        r.register(list_desc());
        r.register(res_desc());
        r.register(input_desc());
        r
    }
}

// ─── Type key ─────────────────────────────────────────────────────────────────

/// Map a Value to its type registry key. Returns "" for types not in the registry
/// (Namespace, NativeFn, Closure, State — these are internal/dynamic and
/// don't have statically-known field/method descriptors).
pub fn value_type_key(v: &Value) -> &'static str {
    match v {
        Value::Float(_)             => "float",
        Value::Bool(_)              => "bool",
        Value::Str(_)               => "string",
        Value::Vec2(..)             => "vec2",
        Value::Vec3(..)             => "vec3",
        Value::Vec4(..)             => "vec4",
        Value::Color { .. }         => "color",
        Value::Mat3(_)              => "mat3",
        Value::Mat4(_)              => "mat4",
        Value::Transform(_)         => "transform",
        Value::Shape(_)             => "shape",
        Value::List(_)              => "list",
        Value::ResOk(_)
        | Value::ResErr(_)          => "res",
        Value::Input { .. }         => "Input",
        // Not in registry — handled specially by the interpreter:
        // Namespace, NativeFn, Closure, State, RenderMode
        _                           => "",
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn float() -> Type { Type::Float }
fn named(s: &str) -> Type { Type::Named(s.into()) }

fn expect_float(v: &Value, name: &str, line: usize) -> Result<f64, RuntimeError> {
    match v {
        Value::Float(x) => Ok(*x),
        _ => Err(RuntimeError::new(line, format!("`{name}` expected float"))),
    }
}

// ─── Primitives ───────────────────────────────────────────────────────────────

fn float_desc() -> TypeDesc {
    TypeDesc { name: "float", fields: vec![], methods: vec![] }
}

fn bool_desc() -> TypeDesc {
    TypeDesc { name: "bool", fields: vec![], methods: vec![] }
}

fn string_desc() -> TypeDesc {
    TypeDesc {
        name: "string",
        fields: vec![
            FieldDesc {
                name: "len",
                ty:   float(),
                get:  |v| { let Value::Str(s) = v else { unreachable!() }; Value::Float(s.len() as f64) },
                set:  None,
            },
        ],
        methods: vec![],
    }
}

// ─── input ────────────────────────────────────────────────────────────────────

fn input_desc() -> TypeDesc {
    TypeDesc {
        name: "Input",
        fields: vec![
            FieldDesc {
                name: "dt",
                ty:   float(),
                get:  |v| { let Value::Input { dt } = v else { unreachable!() }; Value::Float(*dt) },
                set:  None,
            },
        ],
        methods: vec![],
    }
}

// ─── vec2 ─────────────────────────────────────────────────────────────────────

fn vec2_desc() -> TypeDesc {
    TypeDesc {
        name: "vec2",
        fields: vec![
            FieldDesc {
                name: "x",
                ty:   float(),
                get:  |v| { let Value::Vec2(x, _) = v else { unreachable!() }; Value::Float(*x) },
                set:  Some(|v, n| { let Value::Vec2(_, y) = v else { unreachable!() }; let Value::Float(x) = n else { return v }; Value::Vec2(x, y) }),
            },
            FieldDesc {
                name: "y",
                ty:   float(),
                get:  |v| { let Value::Vec2(_, y) = v else { unreachable!() }; Value::Float(*y) },
                set:  Some(|v, n| { let Value::Vec2(x, _) = v else { unreachable!() }; let Value::Float(y) = n else { return v }; Value::Vec2(x, y) }),
            },
        ],
        methods: vec![
            MethodDesc {
                name:   "length",
                params: vec![],
                ret:    Some(float()),
                call:   |v, _args, _line| {
                    let Value::Vec2(x, y) = v else { unreachable!() };
                    Ok(Value::Float((x * x + y * y).sqrt()))
                },
            },
            MethodDesc {
                name:   "normalize",
                params: vec![],
                ret:    Some(named("vec2")),
                call:   |v, _args, line| {
                    let Value::Vec2(x, y) = v else { unreachable!() };
                    let len = (x * x + y * y).sqrt();
                    if len == 0.0 {
                        Err(RuntimeError::new(line, "normalize on zero vector"))
                    } else {
                        Ok(Value::Vec2(x / len, y / len))
                    }
                },
            },
            MethodDesc {
                name:   "dot",
                params: vec![named("vec2")],
                ret:    Some(float()),
                call:   |v, args, line| {
                    let Value::Vec2(ax, ay) = v else { unreachable!() };
                    let Value::Vec2(bx, by) = &args[0] else {
                        return Err(RuntimeError::new(line, "dot expects vec2"));
                    };
                    Ok(Value::Float(ax * bx + ay * by))
                },
            },
            MethodDesc {
                name:   "lerp",
                params: vec![named("vec2"), float()],
                ret:    Some(named("vec2")),
                call:   |v, args, line| {
                    let Value::Vec2(ax, ay) = v else { unreachable!() };
                    let Value::Vec2(bx, by) = &args[0] else {
                        return Err(RuntimeError::new(line, "lerp expects vec2 as first arg"));
                    };
                    let t = expect_float(&args[1], "lerp t", line)?;
                    Ok(Value::Vec2(ax + (bx - ax) * t, ay + (by - ay) * t))
                },
            },
            MethodDesc {
                name:   "distance",
                params: vec![named("vec2")],
                ret:    Some(float()),
                call:   |v, args, line| {
                    let Value::Vec2(ax, ay) = v else { unreachable!() };
                    let Value::Vec2(bx, by) = &args[0] else {
                        return Err(RuntimeError::new(line, "distance expects vec2"));
                    };
                    let dx = ax - bx;
                    let dy = ay - by;
                    Ok(Value::Float((dx * dx + dy * dy).sqrt()))
                },
            },
            MethodDesc {
                name: "abs", params: vec![], ret: Some(named("vec2")),
                call: |v, _args, _line| {
                    let Value::Vec2(x, y) = v else { unreachable!() };
                    Ok(Value::Vec2(x.abs(), y.abs()))
                },
            },
            MethodDesc {
                name: "floor", params: vec![], ret: Some(named("vec2")),
                call: |v, _args, _line| {
                    let Value::Vec2(x, y) = v else { unreachable!() };
                    Ok(Value::Vec2(x.floor(), y.floor()))
                },
            },
            MethodDesc {
                name: "ceil", params: vec![], ret: Some(named("vec2")),
                call: |v, _args, _line| {
                    let Value::Vec2(x, y) = v else { unreachable!() };
                    Ok(Value::Vec2(x.ceil(), y.ceil()))
                },
            },
            MethodDesc {
                name: "min", params: vec![named("vec2")], ret: Some(named("vec2")),
                call: |v, args, line| {
                    let Value::Vec2(ax, ay) = v else { unreachable!() };
                    let Value::Vec2(bx, by) = &args[0] else {
                        return Err(RuntimeError::new(line, "min expects vec2"));
                    };
                    Ok(Value::Vec2(ax.min(*bx), ay.min(*by)))
                },
            },
            MethodDesc {
                name: "max", params: vec![named("vec2")], ret: Some(named("vec2")),
                call: |v, args, line| {
                    let Value::Vec2(ax, ay) = v else { unreachable!() };
                    let Value::Vec2(bx, by) = &args[0] else {
                        return Err(RuntimeError::new(line, "max expects vec2"));
                    };
                    Ok(Value::Vec2(ax.max(*bx), ay.max(*by)))
                },
            },
            MethodDesc {
                name: "perp", params: vec![], ret: Some(named("vec2")),
                call: |v, _args, _line| {
                    let Value::Vec2(x, y) = v else { unreachable!() };
                    Ok(Value::Vec2(-y, *x))
                },
            },
            MethodDesc {
                name: "angle", params: vec![], ret: Some(float()),
                call: |v, _args, _line| {
                    let Value::Vec2(x, y) = v else { unreachable!() };
                    Ok(Value::Float(y.atan2(*x)))
                },
            },
        ],
    }
}

// ─── vec3 ─────────────────────────────────────────────────────────────────────

fn vec3_desc() -> TypeDesc {
    TypeDesc {
        name: "vec3",
        fields: vec![
            FieldDesc {
                name: "x", ty: float(),
                get: |v| { let Value::Vec3(x,_,_) = v else { unreachable!() }; Value::Float(*x) },
                set: Some(|v, n| { let Value::Vec3(_,y,z) = v else { unreachable!() }; let Value::Float(x) = n else { return v }; Value::Vec3(x,y,z) }),
            },
            FieldDesc {
                name: "y", ty: float(),
                get: |v| { let Value::Vec3(_,y,_) = v else { unreachable!() }; Value::Float(*y) },
                set: Some(|v, n| { let Value::Vec3(x,_,z) = v else { unreachable!() }; let Value::Float(y) = n else { return v }; Value::Vec3(x,y,z) }),
            },
            FieldDesc {
                name: "z", ty: float(),
                get: |v| { let Value::Vec3(_,_,z) = v else { unreachable!() }; Value::Float(*z) },
                set: Some(|v, n| { let Value::Vec3(x,y,_) = v else { unreachable!() }; let Value::Float(z) = n else { return v }; Value::Vec3(x,y,z) }),
            },
        ],
        methods: vec![
            MethodDesc {
                name: "length", params: vec![], ret: Some(float()),
                call: |v, _args, _line| {
                    let Value::Vec3(x,y,z) = v else { unreachable!() };
                    Ok(Value::Float((x*x + y*y + z*z).sqrt()))
                },
            },
            MethodDesc {
                name: "normalize", params: vec![], ret: Some(named("vec3")),
                call: |v, _args, line| {
                    let Value::Vec3(x,y,z) = v else { unreachable!() };
                    let len = (x*x + y*y + z*z).sqrt();
                    if len == 0.0 { Err(RuntimeError::new(line, "normalize on zero vector")) }
                    else { Ok(Value::Vec3(x/len, y/len, z/len)) }
                },
            },
            MethodDesc {
                name: "dot", params: vec![named("vec3")], ret: Some(float()),
                call: |v, args, line| {
                    let Value::Vec3(ax,ay,az) = v else { unreachable!() };
                    let Value::Vec3(bx,by,bz) = &args[0] else {
                        return Err(RuntimeError::new(line, "dot expects vec3"));
                    };
                    Ok(Value::Float(ax*bx + ay*by + az*bz))
                },
            },
            MethodDesc {
                name: "cross", params: vec![named("vec3")], ret: Some(named("vec3")),
                call: |v, args, line| {
                    let Value::Vec3(ax,ay,az) = v else { unreachable!() };
                    let Value::Vec3(bx,by,bz) = &args[0] else {
                        return Err(RuntimeError::new(line, "cross expects vec3"));
                    };
                    Ok(Value::Vec3(
                        ay*bz - az*by,
                        az*bx - ax*bz,
                        ax*by - ay*bx,
                    ))
                },
            },
            MethodDesc {
                name: "lerp", params: vec![named("vec3"), float()], ret: Some(named("vec3")),
                call: |v, args, line| {
                    let Value::Vec3(ax,ay,az) = v else { unreachable!() };
                    let Value::Vec3(bx,by,bz) = &args[0] else {
                        return Err(RuntimeError::new(line, "lerp expects vec3 as first arg"));
                    };
                    let t = expect_float(&args[1], "lerp t", line)?;
                    Ok(Value::Vec3(ax+(bx-ax)*t, ay+(by-ay)*t, az+(bz-az)*t))
                },
            },
            MethodDesc {
                name: "abs", params: vec![], ret: Some(named("vec3")),
                call: |v, _args, _line| {
                    let Value::Vec3(x,y,z) = v else { unreachable!() };
                    Ok(Value::Vec3(x.abs(), y.abs(), z.abs()))
                },
            },
            MethodDesc {
                name: "min", params: vec![named("vec3")], ret: Some(named("vec3")),
                call: |v, args, line| {
                    let Value::Vec3(ax,ay,az) = v else { unreachable!() };
                    let Value::Vec3(bx,by,bz) = &args[0] else {
                        return Err(RuntimeError::new(line, "min expects vec3"));
                    };
                    Ok(Value::Vec3(ax.min(*bx), ay.min(*by), az.min(*bz)))
                },
            },
            MethodDesc {
                name: "max", params: vec![named("vec3")], ret: Some(named("vec3")),
                call: |v, args, line| {
                    let Value::Vec3(ax,ay,az) = v else { unreachable!() };
                    let Value::Vec3(bx,by,bz) = &args[0] else {
                        return Err(RuntimeError::new(line, "max expects vec3"));
                    };
                    Ok(Value::Vec3(ax.max(*bx), ay.max(*by), az.max(*bz)))
                },
            },
            MethodDesc {
                name: "reflect", params: vec![named("vec3")], ret: Some(named("vec3")),
                call: |v, args, line| {
                    let Value::Vec3(vx,vy,vz) = v else { unreachable!() };
                    let Value::Vec3(nx,ny,nz) = &args[0] else {
                        return Err(RuntimeError::new(line, "reflect expects vec3 normal"));
                    };
                    let dot2 = 2.0 * (vx*nx + vy*ny + vz*nz);
                    Ok(Value::Vec3(vx - dot2*nx, vy - dot2*ny, vz - dot2*nz))
                },
            },
        ],
    }
}

// ─── vec4 ─────────────────────────────────────────────────────────────────────

fn vec4_desc() -> TypeDesc {
    TypeDesc {
        name: "vec4",
        fields: vec![
            FieldDesc {
                name: "x", ty: float(),
                get: |v| { let Value::Vec4(x,_,_,_) = v else { unreachable!() }; Value::Float(*x) },
                set: Some(|v, n| { let Value::Vec4(_,y,z,w) = v else { unreachable!() }; let Value::Float(x) = n else { return v }; Value::Vec4(x,y,z,w) }),
            },
            FieldDesc {
                name: "y", ty: float(),
                get: |v| { let Value::Vec4(_,y,_,_) = v else { unreachable!() }; Value::Float(*y) },
                set: Some(|v, n| { let Value::Vec4(x,_,z,w) = v else { unreachable!() }; let Value::Float(y) = n else { return v }; Value::Vec4(x,y,z,w) }),
            },
            FieldDesc {
                name: "z", ty: float(),
                get: |v| { let Value::Vec4(_,_,z,_) = v else { unreachable!() }; Value::Float(*z) },
                set: Some(|v, n| { let Value::Vec4(x,y,_,w) = v else { unreachable!() }; let Value::Float(z) = n else { return v }; Value::Vec4(x,y,z,w) }),
            },
            FieldDesc {
                name: "w", ty: float(),
                get: |v| { let Value::Vec4(_,_,_,w) = v else { unreachable!() }; Value::Float(*w) },
                set: Some(|v, n| { let Value::Vec4(x,y,z,_) = v else { unreachable!() }; let Value::Float(w) = n else { return v }; Value::Vec4(x,y,z,w) }),
            },
        ],
        methods: vec![
            MethodDesc {
                name: "length", params: vec![], ret: Some(float()),
                call: |v, _args, _line| {
                    let Value::Vec4(x,y,z,w) = v else { unreachable!() };
                    Ok(Value::Float((x*x + y*y + z*z + w*w).sqrt()))
                },
            },
            MethodDesc {
                name: "normalize", params: vec![], ret: Some(named("vec4")),
                call: |v, _args, line| {
                    let Value::Vec4(x,y,z,w) = v else { unreachable!() };
                    let len = (x*x + y*y + z*z + w*w).sqrt();
                    if len == 0.0 { Err(RuntimeError::new(line, "normalize on zero vector")) }
                    else { Ok(Value::Vec4(x/len, y/len, z/len, w/len)) }
                },
            },
            MethodDesc {
                name: "dot", params: vec![named("vec4")], ret: Some(float()),
                call: |v, args, line| {
                    let Value::Vec4(ax,ay,az,aw) = v else { unreachable!() };
                    let Value::Vec4(bx,by,bz,bw) = &args[0] else {
                        return Err(RuntimeError::new(line, "dot expects vec4"));
                    };
                    Ok(Value::Float(ax*bx + ay*by + az*bz + aw*bw))
                },
            },
            MethodDesc {
                name: "lerp", params: vec![named("vec4"), float()], ret: Some(named("vec4")),
                call: |v, args, line| {
                    let Value::Vec4(ax,ay,az,aw) = v else { unreachable!() };
                    let Value::Vec4(bx,by,bz,bw) = &args[0] else {
                        return Err(RuntimeError::new(line, "lerp expects vec4 as first arg"));
                    };
                    let t = expect_float(&args[1], "lerp t", line)?;
                    Ok(Value::Vec4(ax+(bx-ax)*t, ay+(by-ay)*t, az+(bz-az)*t, aw+(bw-aw)*t))
                },
            },
            MethodDesc {
                name: "abs", params: vec![], ret: Some(named("vec4")),
                call: |v, _args, _line| {
                    let Value::Vec4(x,y,z,w) = v else { unreachable!() };
                    Ok(Value::Vec4(x.abs(), y.abs(), z.abs(), w.abs()))
                },
            },
            MethodDesc {
                name: "min", params: vec![named("vec4")], ret: Some(named("vec4")),
                call: |v, args, line| {
                    let Value::Vec4(ax,ay,az,aw) = v else { unreachable!() };
                    let Value::Vec4(bx,by,bz,bw) = &args[0] else {
                        return Err(RuntimeError::new(line, "min expects vec4"));
                    };
                    Ok(Value::Vec4(ax.min(*bx), ay.min(*by), az.min(*bz), aw.min(*bw)))
                },
            },
            MethodDesc {
                name: "max", params: vec![named("vec4")], ret: Some(named("vec4")),
                call: |v, args, line| {
                    let Value::Vec4(ax,ay,az,aw) = v else { unreachable!() };
                    let Value::Vec4(bx,by,bz,bw) = &args[0] else {
                        return Err(RuntimeError::new(line, "max expects vec4"));
                    };
                    Ok(Value::Vec4(ax.max(*bx), ay.max(*by), az.max(*bz), aw.max(*bw)))
                },
            },
        ],
    }
}

// ─── color ────────────────────────────────────────────────────────────────────

fn color_desc() -> TypeDesc {
    TypeDesc {
        name: "color",
        fields: vec![
            FieldDesc {
                name: "r", ty: float(),
                get: |v| { let Value::Color { r, .. } = v else { unreachable!() }; Value::Float(*r) },
                set: Some(|v, n| { let Value::Color { g, b, a, .. } = v else { unreachable!() }; let Value::Float(r) = n else { return v }; Value::Color { r, g, b, a } }),
            },
            FieldDesc {
                name: "g", ty: float(),
                get: |v| { let Value::Color { g, .. } = v else { unreachable!() }; Value::Float(*g) },
                set: Some(|v, n| { let Value::Color { r, b, a, .. } = v else { unreachable!() }; let Value::Float(g) = n else { return v }; Value::Color { r, g, b, a } }),
            },
            FieldDesc {
                name: "b", ty: float(),
                get: |v| { let Value::Color { b, .. } = v else { unreachable!() }; Value::Float(*b) },
                set: Some(|v, n| { let Value::Color { r, g, a, .. } = v else { unreachable!() }; let Value::Float(b) = n else { return v }; Value::Color { r, g, b, a } }),
            },
            FieldDesc {
                name: "a", ty: float(),
                get: |v| { let Value::Color { a, .. } = v else { unreachable!() }; Value::Float(*a) },
                set: Some(|v, n| { let Value::Color { r, g, b, .. } = v else { unreachable!() }; let Value::Float(a) = n else { return v }; Value::Color { r, g, b, a } }),
            },
        ],
        methods: vec![
            MethodDesc {
                name: "lerp", params: vec![named("color"), float()], ret: Some(named("color")),
                call: |v, args, line| {
                    let Value::Color { r: ar, g: ag, b: ab, a: aa } = v else { unreachable!() };
                    let Value::Color { r: br, g: bg, b: bb, a: ba } = &args[0] else {
                        return Err(RuntimeError::new(line, "lerp expects color as first arg"));
                    };
                    let t = expect_float(&args[1], "lerp t", line)?;
                    Ok(Value::Color {
                        r: ar + (br - ar) * t,
                        g: ag + (bg - ag) * t,
                        b: ab + (bb - ab) * t,
                        a: aa + (ba - aa) * t,
                    })
                },
            },
            MethodDesc {
                name: "with_alpha", params: vec![float()], ret: Some(named("color")),
                call: |v, args, line| {
                    let Value::Color { r, g, b, .. } = v else { unreachable!() };
                    let a = expect_float(&args[0], "with_alpha a", line)?;
                    Ok(Value::Color { r: *r, g: *g, b: *b, a })
                },
            },
            MethodDesc {
                name: "to_vec4", params: vec![], ret: Some(named("vec4")),
                call: |v, _args, _line| {
                    let Value::Color { r, g, b, a } = v else { unreachable!() };
                    Ok(Value::Vec4(*r, *g, *b, *a))
                },
            },
        ],
    }
}

// ─── transform ────────────────────────────────────────────────────────────────

fn transform_desc() -> TypeDesc {
    TypeDesc {
        name: "transform",
        fields: vec![],
        methods: vec![
            MethodDesc {
                name: "move", params: vec![float(), float()], ret: Some(named("transform")),
                call: |v, args, line| {
                    let Value::Transform(td) = v else { unreachable!() };
                    let dx = expect_float(&args[0], "move dx", line)?;
                    let dy = expect_float(&args[1], "move dy", line)?;
                    let mut t = td.clone();
                    t.tx += dx;
                    t.ty += dy;
                    Ok(Value::Transform(t))
                },
            },
            MethodDesc {
                name: "translate", params: vec![float(), float()], ret: Some(named("transform")),
                call: |v, args, line| {
                    let Value::Transform(td) = v else { unreachable!() };
                    let dx = expect_float(&args[0], "translate dx", line)?;
                    let dy = expect_float(&args[1], "translate dy", line)?;
                    let mut t = td.clone();
                    t.tx += dx;
                    t.ty += dy;
                    Ok(Value::Transform(t))
                },
            },
            MethodDesc {
                name: "scale", params: vec![float()], ret: Some(named("transform")),
                call: |v, args, line| {
                    let Value::Transform(td) = v else { unreachable!() };
                    let s = expect_float(&args[0], "scale s", line)?;
                    let mut t = td.clone();
                    t.sx *= s;
                    t.sy *= s;
                    Ok(Value::Transform(t))
                },
            },
            MethodDesc {
                name: "rotate", params: vec![float()], ret: Some(named("transform")),
                call: |v, args, line| {
                    let Value::Transform(td) = v else { unreachable!() };
                    let deg = expect_float(&args[0], "rotate degrees", line)?;
                    let mut t = td.clone();
                    t.angle += deg.to_radians();
                    Ok(Value::Transform(t))
                },
            },
        ],
    }
}

// ─── shape ────────────────────────────────────────────────────────────────────

fn shape_desc() -> TypeDesc {
    TypeDesc {
        name: "shape",
        fields: vec![],
        methods: vec![
            MethodDesc {
                name: "in", params: vec![float(), float()], ret: Some(named("vec2")),
                call: |v, args, line| {
                    let Value::Shape(shape) = v else { unreachable!() };
                    let dx = expect_float(&args[0], "in dx", line)?;
                    let dy = expect_float(&args[1], "in dy", line)?;
                    let (ax, ay) = shape.desc.anchor();
                    Ok(Value::Vec2(ax + dx, ay + dy))
                },
            },
        ],
    }
}

// ─── list ─────────────────────────────────────────────────────────────────────

fn list_desc() -> TypeDesc {
    TypeDesc {
        name: "list",
        fields: vec![
            FieldDesc {
                name: "len", ty: float(),
                get: |v| { let Value::List(items) = v else { unreachable!() }; Value::Float(items.borrow().len() as f64) },
                set: None,
            },
        ],
        methods: vec![
            MethodDesc {
                name: "len", params: vec![], ret: Some(float()),
                call: |v, _args, _line| {
                    let Value::List(items) = v else { unreachable!() };
                    Ok(Value::Float(items.borrow().len() as f64))
                },
            },
            MethodDesc {
                // Mutates the shared list in-place through the Rc. Returns unit.
                name: "push", params: vec![Type::Float], // placeholder — resolved generically
                ret: None,
                call: |v, args, _line| {
                    let Value::List(items) = v else { unreachable!() };
                    if let Some(item) = args.first() {
                        items.borrow_mut().push(item.clone());
                    }
                    Ok(Value::List(items.clone()))
                },
            },
            MethodDesc {
                // Removes and returns the last element. Mutates in-place.
                name: "pop", params: vec![], ret: Some(Type::Float), // placeholder
                call: |v, _args, line| {
                    let Value::List(items) = v else { unreachable!() };
                    items.borrow_mut().pop()
                        .ok_or_else(|| RuntimeError::new(line, "pop on empty list"))
                },
            },
        ],
    }
}

// ─── res ──────────────────────────────────────────────────────────────────────

fn res_desc() -> TypeDesc {
    TypeDesc {
        name: "res",
        fields: vec![
            FieldDesc {
                name: "ok", ty: Type::Bool,
                get: |v| match v {
                    Value::ResOk(_)  => Value::Bool(true),
                    Value::ResErr(_) => Value::Bool(false),
                    _ => unreachable!(),
                },
                set: None,
            },
            FieldDesc {
                name: "value", ty: Type::Float, // placeholder — inner type unknown at registry level
                get: |v| {
                    let Value::ResOk(inner) = v else { unreachable!() };
                    *inner.clone()
                },
                set: None,
            },
            FieldDesc {
                name: "error", ty: named("string"),
                get: |v| {
                    let Value::ResErr(s) = v else { unreachable!() };
                    Value::Str(s.clone())
                },
                set: None,
            },
        ],
        methods: vec![],
    }
}

// ─── mat3 ─────────────────────────────────────────────────────────────────────

fn mat3_desc() -> TypeDesc {
    use crate::types::mat::*;
    TypeDesc {
        name: "mat3",
        fields: vec![],
        methods: vec![
            MethodDesc {
                name: "transpose", params: vec![], ret: Some(named("mat3")),
                call: |v, _args, _line| {
                    let Value::Mat3(m) = v else { unreachable!() };
                    Ok(Value::Mat3(Box::new(m3_transpose(m))))
                },
            },
            MethodDesc {
                name: "det", params: vec![], ret: Some(float()),
                call: |v, _args, _line| {
                    let Value::Mat3(m) = v else { unreachable!() };
                    Ok(Value::Float(m3_det(m)))
                },
            },
            MethodDesc {
                name: "inverse", params: vec![], ret: Some(named("mat3")),
                call: |v, _args, line| {
                    let Value::Mat3(m) = v else { unreachable!() };
                    Ok(Value::Mat3(Box::new(m3_inverse(m, line)?)))
                },
            },
            MethodDesc {
                name: "mul_vec", params: vec![named("vec3")], ret: Some(named("vec3")),
                call: |v, args, line| {
                    let Value::Mat3(m) = v else { unreachable!() };
                    let Value::Vec3(x, y, z) = &args[0] else {
                        return Err(RuntimeError::new(line, "mul_vec expects vec3"));
                    };
                    let (rx, ry, rz) = m3_mul_vec(m, (*x, *y, *z));
                    Ok(Value::Vec3(rx, ry, rz))
                },
            },
            MethodDesc {
                name: "scale", params: vec![float()], ret: Some(named("mat3")),
                call: |v, args, line| {
                    let Value::Mat3(m) = v else { unreachable!() };
                    let s = expect_float(&args[0], "scale s", line)?;
                    Ok(Value::Mat3(Box::new(m3_scale(m, s))))
                },
            },
        ],
    }
}

// ─── mat4 ─────────────────────────────────────────────────────────────────────

fn mat4_desc() -> TypeDesc {
    use crate::types::mat::*;
    TypeDesc {
        name: "mat4",
        fields: vec![],
        methods: vec![
            MethodDesc {
                name: "transpose", params: vec![], ret: Some(named("mat4")),
                call: |v, _args, _line| {
                    let Value::Mat4(m) = v else { unreachable!() };
                    Ok(Value::Mat4(Box::new(m4_transpose(m))))
                },
            },
            MethodDesc {
                name: "det", params: vec![], ret: Some(float()),
                call: |v, _args, _line| {
                    let Value::Mat4(m) = v else { unreachable!() };
                    Ok(Value::Float(m4_det(m)))
                },
            },
            MethodDesc {
                name: "inverse", params: vec![], ret: Some(named("mat4")),
                call: |v, _args, line| {
                    let Value::Mat4(m) = v else { unreachable!() };
                    Ok(Value::Mat4(Box::new(m4_inverse(m, line)?)))
                },
            },
            MethodDesc {
                name: "mul_vec", params: vec![named("vec4")], ret: Some(named("vec4")),
                call: |v, args, line| {
                    let Value::Mat4(m) = v else { unreachable!() };
                    let Value::Vec4(x, y, z, w) = &args[0] else {
                        return Err(RuntimeError::new(line, "mul_vec expects vec4"));
                    };
                    let (rx, ry, rz, rw) = m4_mul_vec(m, (*x, *y, *z, *w));
                    Ok(Value::Vec4(rx, ry, rz, rw))
                },
            },
            MethodDesc {
                name: "scale", params: vec![float()], ret: Some(named("mat4")),
                call: |v, args, line| {
                    let Value::Mat4(m) = v else { unreachable!() };
                    let s = expect_float(&args[0], "scale s", line)?;
                    Ok(Value::Mat4(Box::new(m4_scale(m, s))))
                },
            },
        ],
    }
}

