use crate::error::AnyError;
use crate::{OpFn, OpState};

pub type SourcePair = (&'static str, &'static str);
pub type OpPair = (&'static str, Box<OpFn>);
pub type OpMiddlewareFn = dyn Fn(&'static str, Box<OpFn>) -> Box<OpFn>;
pub type OpStateFn = dyn Fn(&mut OpState) -> Result<(), AnyError>;

#[derive(Default)]
pub struct Extension {
  js_files: Option<Vec<SourcePair>>,
  ops: Option<Vec<OpPair>>,
  opstate_fn: Option<Box<OpStateFn>>,
  middleware_fn: Option<Box<OpMiddlewareFn>>,
  initialized: bool,
}

// Note: this used to be a trait, but we "downgraded" it to a single concrete type
// for the initial iteration, it will likely become a trait in the future
impl Extension {
  pub fn builder() -> ExtensionBuilder {
    Default::default()
  }

  /// returns JS source code to be loaded into the isolate (either at snapshotting,
  /// or at startup).  as a vector of a tuple of the file name, and the source code.
  pub(crate) fn init_js(&self) -> Vec<SourcePair> {
    match &self.js_files {
      Some(files) => files.clone(),
      None => vec![],
    }
  }

  /// Called at JsRuntime startup to initialize ops in the isolate.
  pub(crate) fn init_ops(&mut self) -> Option<Vec<OpPair>> {
    // TODO(@AaronO): maybe make op registration idempotent
    if self.initialized {
      panic!("init_ops called twice: not idempotent or correct");
    }
    self.initialized = true;

    self.ops.take()
  }

  /// Allows setting up the initial op-state of an isolate at startup.
  pub(crate) fn init_state(&self, state: &mut OpState) -> Result<(), AnyError> {
    match &self.opstate_fn {
      Some(ofn) => ofn(state),
      None => Ok(()),
    }
  }

  /// init_middleware lets us middleware op registrations, it's called before init_ops
  pub(crate) fn init_middleware(&mut self) -> Option<Box<OpMiddlewareFn>> {
    self.middleware_fn.take()
  }
}

// Provides a convenient builder pattern to declare Extensions
#[derive(Default)]
pub struct ExtensionBuilder {
  js: Vec<SourcePair>,
  ops: Vec<OpPair>,
  state: Option<Box<OpStateFn>>,
  middleware: Option<Box<OpMiddlewareFn>>,
}

impl ExtensionBuilder {
  pub fn js(&mut self, js_files: Vec<SourcePair>) -> &mut Self {
    self.js.extend(js_files);
    self
  }

  pub fn ops(&mut self, ops: Vec<OpPair>) -> &mut Self {
    self.ops.extend(ops);
    self
  }

  pub fn state<F>(&mut self, opstate_fn: F) -> &mut Self
  where
    F: Fn(&mut OpState) -> Result<(), AnyError> + 'static,
  {
    self.state = Some(Box::new(opstate_fn));
    self
  }

  pub fn middleware<F>(&mut self, middleware_fn: F) -> &mut Self
  where
    F: Fn(&'static str, Box<OpFn>) -> Box<OpFn> + 'static,
  {
    self.middleware = Some(Box::new(middleware_fn));
    self
  }

  pub fn build(&mut self) -> Extension {
    let js_files = Some(std::mem::take(&mut self.js));
    let ops = Some(std::mem::take(&mut self.ops));
    Extension {
      js_files,
      ops,
      opstate_fn: self.state.take(),
      middleware_fn: self.middleware.take(),
      initialized: false,
    }
  }
}
/// Helps embed JS files in an extension. Returns Vec<(&'static str, &'static str)>
/// representing the filename and source code.
///
/// Example:
/// ```ignore
/// include_js_files!(
///   prefix "deno:op_crates/hello",
///   "01_hello.js",
///   "02_goodbye.js",
/// )
/// ```
#[macro_export]
macro_rules! include_js_files {
  (prefix $prefix:literal, $($file:literal,)+) => {
    vec![
      $((
        concat!($prefix, "/", $file),
        include_str!($file),
      ),)+
    ]
  };
}

// declare_ops! helps declare ops for an extension.
// Example:
// ```
// declare_ops!(
//   sync[
//     op_foo,
//     op_bar,
//   ],
//   async[
//     op_write,
//     op_read,
//   ],
// )
// ```
#[macro_export]
macro_rules! declare_ops {
  // A flattened group of async[] & sync[] subgroups
  ($($wrapper:ident[$($opfn:expr,)+],)+) => {
    vec![
      $(declare_ops!($wrapper[$($opfn,)+]),)+
    ].into_iter().flatten().collect()
  };

  // Async group
  (async[$($opfn:expr,)+]) => {
    vec![$((
      $crate::extensions::op_ident(stringify!($opfn)),
      $crate::op_async($opfn),
    ),)+]
  };

  // Sync group
  (sync[$($opfn:expr,)+]) => {
    vec![$((
      $crate::extensions::op_ident(stringify!($opfn)),
      $crate::op_sync($opfn),
    ),)+]
  };
}

/// transforms a stringified identifier path into an op_name
/// it also enforces that all op_names must start with "op_"
// stringify!(foo::op_bar<X>) => "foo :: op_bar < X >"
// op_ident(stringify!(foo::op_bar<X>)) => "op_bar"
pub fn op_ident(ident_path: &'static str) -> &'static str {
  let end = ident_path.rfind("::<").unwrap_or_else(|| ident_path.len());
  let ident_path = ident_path.get(0..end).unwrap();
  let start = ident_path.rfind("::").map(|i| i + 2).unwrap_or(0);
  let name = ident_path.get(start..ident_path.len()).unwrap();

  // Assert op_ prefix
  assert!(
    name.starts_with("op_"),
    "Op '{}' missing 'op_' prefix",
    name
  );

  name
}
