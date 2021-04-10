use rusty_v8 as v8;
use std::cell::RefCell;

pub enum SerializablePkg {
  MinValue(MinValue),
  Serializable(Box<dyn serde_v8::Serializable>),
}

impl SerializablePkg {
  pub fn to_v8<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
  ) -> Result<v8::Local<'a, v8::Value>, serde_v8::Error> {
    
    match &*self {
      Self::MinValue(x) => serde_v8::to_v8(scope, x),
      Self::Serializable(x) => x.to_v8(scope),
    }
  }
}

/// MinValue serves as a lightweight serializable wrapper around primitives
/// so that we can use them for async values
#[derive(Clone, Copy)]
pub enum MinValue {
  Unit(()),
  Bool(bool),
  Int8(i8),
  Int16(i16),
  Int32(i32),
  Int64(i64),
  UInt8(u8),
  UInt16(u16),
  UInt32(u32),
  UInt64(u64),
  Float32(f32),
  Float64(f64),
}

impl serde::Serialize for MinValue {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
      match *self {
        Self::Unit(_) => serializer.serialize_unit(),
        Self::Bool(x) => serializer.serialize_bool(x),
        Self::Int8(x) => serializer.serialize_i8(x),
        Self::Int16(x) => serializer.serialize_i16(x),
        Self::Int32(x) => serializer.serialize_i32(x),
        Self::Int64(x) => serializer.serialize_i64(x),
        Self::UInt8(x) => serializer.serialize_u8(x),
        Self::UInt16(x) => serializer.serialize_u16(x),
        Self::UInt32(x) => serializer.serialize_u32(x),
        Self::UInt64(x) => serializer.serialize_u64(x),
        Self::Float32(x) => serializer.serialize_f32(x),
        Self::Float64(x) => serializer.serialize_f64(x),
      }
  }
}

////
// A specialization hack, see:
// https://lukaskalbertodt.github.io/2019/12/05/generalized-autoref-based-specialization.html
////

pub fn to_pkg<T: serde::Serialize + Default + 'static>(x: T) -> SerializablePkg {
  (&&Wrap(RefCell::new(x))).to_pkg()
}

struct Wrap<T>(RefCell<T>);
trait ViaPrimitive { fn to_pkg(&self) -> SerializablePkg; }

macro_rules! impl_via_primitive {
  ($($T:ty => $minval:ident,)+) => {
    $(impl ViaPrimitive for &&Wrap<$T> {
      fn to_pkg(&self) -> SerializablePkg {
        SerializablePkg::MinValue(MinValue::$minval(self.0.take()))
      }
    })+
  };
}

impl_via_primitive!(
  () => Unit,
  bool => Bool,
  i8 => Int8,
  i16 => Int16,
  i32 => Int32,
  i64 => Int64,
  u8 => UInt8,
  u16 => UInt16,
  u32 => UInt32,
  u64 => UInt64,
  f32 => Float32,
  f64 => Float64,
);

trait ViaSerializable { fn to_pkg(&self) -> SerializablePkg; }
impl<T: serde::Serialize + Default + 'static> ViaSerializable for &Wrap<T> {
  fn to_pkg(&self) -> SerializablePkg {
    SerializablePkg::Serializable(Box::new(self.0.take()))
  }
}
