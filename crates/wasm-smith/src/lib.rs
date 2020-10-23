//! A WebAssembly test case generator.
//!
//! ## Usage
//!
//! First, use [`cargo fuzz`](https://github.com/rust-fuzz/cargo-fuzz) to define
//! a new fuzz target:
//!
//! ```shell
//! $ cargo fuzz add my_wasm_smith_fuzz_target
//! ```
//!
//! Next, add `wasm-smith` to your dependencies:
//!
//! ```toml
//! # fuzz/Cargo.toml
//!
//! [dependencies]
//! wasm-smith = "0.1.5"
//! ```
//!
//! Then, define your fuzz target so that it takes arbitrary
//! `wasm_smith::Module`s as an argument, convert the module into serialized
//! Wasm bytes via the `to_bytes` method, and then feed it into your system:
//!
//! ```no_run
//! // fuzz/fuzz_targets/my_wasm_smith_fuzz_target.rs
//!
//! #![no_main]
//!
//! use libfuzzer_sys::fuzz_target;
//! use wasm_smith::Module;
//!
//! fuzz_target!(|module: Module| {
//!     let wasm_bytes = module.to_bytes();
//!
//!     // Your code here...
//! });
//! ```
//!
//! Finally, start fuzzing:
//!
//! ```shell
//! $ cargo fuzz run my_wasm_smith_fuzz_target
//! ```
//!
//! > **Note:** For a real world example, also check out [the `validate` fuzz
//! > target](https://github.com/fitzgen/wasm-smith/blob/main/fuzz/fuzz_targets/validate.rs)
//! > defined in this repository. Using the `wasmparser` crate, it checks that
//! > every module generated by `wasm-smith` validates successfully.

#![deny(missing_docs, missing_debug_implementations)]
// Needed for the `instructions!` macro in `src/code_builder.rs`.
#![recursion_limit = "256"]

mod code_builder;
mod config;
mod encode;
mod terminate;

use crate::code_builder::CodeBuilderAllocations;
use arbitrary::{Arbitrary, Result, Unstructured};
use std::collections::HashSet;
use std::str;

pub use config::{Config, DefaultConfig, SwarmConfig};

/// A pseudo-random WebAssembly module.
///
/// Construct instances of this type with [the `Arbitrary`
/// trait](https://docs.rs/arbitrary/*/arbitrary/trait.Arbitrary.html).
///
/// ## Configuring Generated Modules
///
/// This uses the [`DefaultConfig`][crate::DefaultConfig] configuration. If you
/// want to customize the shape of generated modules, define your own
/// configuration type, implement the [`Config`][crate::Config] trait for it,
/// and use [`ConfiguredModule<YourConfigType>`][crate::ConfiguredModule]
/// instead of plain `Module`.
#[derive(Debug, Default, Arbitrary)]
pub struct Module {
    inner: ConfiguredModule<DefaultConfig>,
}

/// A pseudo-random generated WebAssembly file with custom configuration.
///
/// If you don't care about custom configuration, use [`Module`][crate::Module]
/// instead.
///
/// For details on configuring, see the [`Config`][crate::Config] trait.
#[derive(Debug, Default)]
pub struct ConfiguredModule<C>
where
    C: Config,
{
    config: C,
    valtypes: Vec<ValType>,
    types: Vec<FuncType>,
    imports: Vec<(String, String, Import)>,
    funcs: Vec<u32>,
    tables: Vec<TableType>,
    memories: Vec<MemoryType>,
    globals: Vec<Global>,
    exports: Vec<(String, Export)>,
    start: Option<u32>,
    elems: Vec<ElementSegment>,
    code: Vec<Code>,
    data: Vec<DataSegment>,
}

impl<C: Config> ConfiguredModule<C> {
    /// Returns a reference to the internal configuration.
    pub fn config(&self) -> &C {
        &self.config
    }
}

impl<C: Config> Arbitrary for ConfiguredModule<C> {
    fn arbitrary(u: &mut Unstructured) -> Result<Self> {
        let mut module = ConfiguredModule::<C>::default();
        module.build(u, false)?;
        Ok(module)
    }
}

/// Same as [`Module`], but may be invalid.
///
/// This module generates function bodies differnetly than `Module` to try to
/// better explore wasm decoders and such.
#[derive(Debug, Default)]
pub struct MaybeInvalidModule {
    module: Module,
}

impl MaybeInvalidModule {
    /// Encode this Wasm module into bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        self.module.to_bytes()
    }
}

impl Arbitrary for MaybeInvalidModule {
    fn arbitrary(u: &mut Unstructured) -> Result<Self> {
        let mut module = Module::default();
        module.inner.build(u, true)?;
        Ok(MaybeInvalidModule { module })
    }
}

#[derive(Clone, Debug)]
struct FuncType {
    params: Vec<ValType>,
    results: Vec<ValType>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum ValType {
    I32,
    I64,
    F32,
    F64,
    FuncRef,
    ExternRef,
}

#[derive(Clone, Debug)]
enum Import {
    Func(u32),
    Table(TableType),
    Memory(MemoryType),
    Global(GlobalType),
}

#[derive(Clone, Debug)]
struct TableType {
    limits: Limits,
    elem_ty: ValType,
}

#[derive(Clone, Debug)]
struct MemoryType {
    limits: Limits,
}

impl Arbitrary for MemoryType {
    fn arbitrary(u: &mut Unstructured<'_>) -> Result<Self> {
        let limits = Limits::limited(u, 65536)?;
        Ok(MemoryType { limits })
    }
}

#[derive(Clone, Debug)]
struct Limits {
    min: u32,
    max: Option<u32>,
}

impl Limits {
    fn limited(u: &mut Unstructured, max: u32) -> Result<Self> {
        let min = u.int_in_range(0..=max)?;
        let max = if u.arbitrary().unwrap_or(false) {
            Some(if min == max {
                max
            } else {
                u.int_in_range(min..=max)?
            })
        } else {
            None
        };
        Ok(Limits { min, max })
    }
}

#[derive(Clone, Debug)]
struct Global {
    ty: GlobalType,
    expr: Instruction,
}

#[derive(Clone, Debug)]
struct GlobalType {
    val_type: ValType,
    mutable: bool,
}

#[derive(Clone, Debug)]
enum Export {
    Func(u32),
    Table(u32),
    Memory(u32),
    Global(u32),
}

#[derive(Debug)]
struct ElementSegment {
    kind: ElementKind,
    ty: ValType,
    items: Elements,
}

#[derive(Debug)]
enum ElementKind {
    Passive,
    Declared,
    Active {
        table: Option<u32>, // None == table 0 implicitly
        offset: Instruction,
    },
}

#[derive(Debug)]
enum Elements {
    Functions(Vec<u32>),
    Expressions(Vec<Option<u32>>),
}

#[derive(Debug)]
struct Code {
    locals: Vec<ValType>,
    instructions: Instructions,
}

#[derive(Debug)]
enum Instructions {
    Generated(Vec<Instruction>),
    Arbitrary(Vec<u8>),
}

#[derive(Clone, Copy, Debug)]
enum BlockType {
    Empty,
    Result(ValType),
    FuncType(u32),
}

impl BlockType {
    fn params_results<C>(&self, module: &ConfiguredModule<C>) -> (Vec<ValType>, Vec<ValType>)
    where
        C: Config,
    {
        match self {
            BlockType::Empty => (vec![], vec![]),
            BlockType::Result(t) => (vec![], vec![*t]),
            BlockType::FuncType(ty) => {
                let ty = &module.types[*ty as usize];
                (ty.params.clone(), ty.results.clone())
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct MemArg {
    offset: u32,
    align: u32,
    memory_index: u32,
}

#[derive(Clone, Debug)]
#[allow(non_camel_case_types)]
enum Instruction {
    // Control instructions.
    Unreachable,
    Nop,
    Block(BlockType),
    Loop(BlockType),
    If(BlockType),
    Else,
    End,
    Br(u32),
    BrIf(u32),
    BrTable(Vec<u32>, u32),
    Return,
    Call(u32),
    CallIndirect { ty: u32, table: u32 },

    // Parametric instructions.
    Drop,
    Select,

    // Variable instructions.
    LocalGet(u32),
    LocalSet(u32),
    LocalTee(u32),
    GlobalGet(u32),
    GlobalSet(u32),

    // Memory instructions.
    I32Load(MemArg),
    I64Load(MemArg),
    F32Load(MemArg),
    F64Load(MemArg),
    I32Load8_S(MemArg),
    I32Load8_U(MemArg),
    I32Load16_S(MemArg),
    I32Load16_U(MemArg),
    I64Load8_S(MemArg),
    I64Load8_U(MemArg),
    I64Load16_S(MemArg),
    I64Load16_U(MemArg),
    I64Load32_S(MemArg),
    I64Load32_U(MemArg),
    I32Store(MemArg),
    I64Store(MemArg),
    F32Store(MemArg),
    F64Store(MemArg),
    I32Store8(MemArg),
    I32Store16(MemArg),
    I64Store8(MemArg),
    I64Store16(MemArg),
    I64Store32(MemArg),
    MemorySize(u32),
    MemoryGrow(u32),
    MemoryInit { mem: u32, data: u32 },
    DataDrop(u32),
    MemoryCopy { src: u32, dst: u32 },
    MemoryFill(u32),

    // Numeric instructions.
    I32Const(i32),
    I64Const(i64),
    F32Const(f32),
    F64Const(f64),
    I32Eqz,
    I32Eq,
    I32Neq,
    I32LtS,
    I32LtU,
    I32GtS,
    I32GtU,
    I32LeS,
    I32LeU,
    I32GeS,
    I32GeU,
    I64Eqz,
    I64Eq,
    I64Neq,
    I64LtS,
    I64LtU,
    I64GtS,
    I64GtU,
    I64LeS,
    I64LeU,
    I64GeS,
    I64GeU,
    F32Eq,
    F32Neq,
    F32Lt,
    F32Gt,
    F32Le,
    F32Ge,
    F64Eq,
    F64Neq,
    F64Lt,
    F64Gt,
    F64Le,
    F64Ge,
    I32Clz,
    I32Ctz,
    I32Popcnt,
    I32Add,
    I32Sub,
    I32Mul,
    I32DivS,
    I32DivU,
    I32RemS,
    I32RemU,
    I32And,
    I32Or,
    I32Xor,
    I32Shl,
    I32ShrS,
    I32ShrU,
    I32Rotl,
    I32Rotr,
    I64Clz,
    I64Ctz,
    I64Popcnt,
    I64Add,
    I64Sub,
    I64Mul,
    I64DivS,
    I64DivU,
    I64RemS,
    I64RemU,
    I64And,
    I64Or,
    I64Xor,
    I64Shl,
    I64ShrS,
    I64ShrU,
    I64Rotl,
    I64Rotr,
    F32Abs,
    F32Neg,
    F32Ceil,
    F32Floor,
    F32Trunc,
    F32Nearest,
    F32Sqrt,
    F32Add,
    F32Sub,
    F32Mul,
    F32Div,
    F32Min,
    F32Max,
    F32Copysign,
    F64Abs,
    F64Neg,
    F64Ceil,
    F64Floor,
    F64Trunc,
    F64Nearest,
    F64Sqrt,
    F64Add,
    F64Sub,
    F64Mul,
    F64Div,
    F64Min,
    F64Max,
    F64Copysign,
    I32WrapI64,
    I32TruncF32S,
    I32TruncF32U,
    I32TruncF64S,
    I32TruncF64U,
    I64ExtendI32S,
    I64ExtendI32U,
    I64TruncF32S,
    I64TruncF32U,
    I64TruncF64S,
    I64TruncF64U,
    F32ConvertI32S,
    F32ConvertI32U,
    F32ConvertI64S,
    F32ConvertI64U,
    F32DemoteF64,
    F64ConvertI32S,
    F64ConvertI32U,
    F64ConvertI64S,
    F64ConvertI64U,
    F64PromoteF32,
    I32ReinterpretF32,
    I64ReinterpretF64,
    F32ReinterpretI32,
    F64ReinterpretI64,
    I32Extend8S,
    I32Extend16S,
    I64Extend8S,
    I64Extend16S,
    I64Extend32S,
    I32TruncSatF32S,
    I32TruncSatF32U,
    I32TruncSatF64S,
    I32TruncSatF64U,
    I64TruncSatF32S,
    I64TruncSatF32U,
    I64TruncSatF64S,
    I64TruncSatF64U,
    TypedSelect(ValType),
    RefNull(ValType),
    RefIsNull,
    RefFunc(u32),
    TableInit { segment: u32, table: u32 },
    ElemDrop { segment: u32 },
    TableFill { table: u32 },
    TableSet { table: u32 },
    TableGet { table: u32 },
    TableGrow { table: u32 },
    TableSize { table: u32 },
    TableCopy { src: u32, dst: u32 },
}

#[derive(Debug)]
struct DataSegment {
    kind: DataSegmentKind,
    init: Vec<u8>,
}

#[derive(Debug)]
enum DataSegmentKind {
    Passive,
    Active {
        memory_index: u32,
        offset: Instruction,
    },
}

impl<C> ConfiguredModule<C>
where
    C: Config,
{
    fn build(&mut self, u: &mut Unstructured, allow_invalid: bool) -> Result<()> {
        self.config = C::arbitrary(u)?;
        self.valtypes.push(ValType::I32);
        self.valtypes.push(ValType::I64);
        self.valtypes.push(ValType::F32);
        self.valtypes.push(ValType::F64);
        if self.config.reference_types_enabled() {
            self.valtypes.push(ValType::ExternRef);
            self.valtypes.push(ValType::FuncRef);
        }
        self.arbitrary_types(u)?;
        self.arbitrary_imports(u)?;
        self.arbitrary_funcs(u)?;
        self.arbitrary_tables(u)?;
        self.arbitrary_memories(u)?;
        self.arbitrary_globals(u)?;
        self.arbitrary_exports(u)?;
        self.arbitrary_start(u)?;
        self.arbitrary_elems(u)?;
        self.arbitrary_data(u)?;
        self.arbitrary_code(u, allow_invalid)?;
        Ok(())
    }

    fn arbitrary_types(&mut self, u: &mut Unstructured) -> Result<()> {
        arbitrary_loop(u, self.config.max_types(), |u| {
            let mut params = vec![];
            let mut results = vec![];
            arbitrary_loop(u, 20, |u| {
                params.push(self.arbitrary_valtype(u)?);
                Ok(())
            })?;
            arbitrary_loop(u, 20, |u| {
                results.push(self.arbitrary_valtype(u)?);
                Ok(())
            })?;
            self.types.push(FuncType { params, results });
            Ok(())
        })
    }

    fn arbitrary_imports(&mut self, u: &mut Unstructured) -> Result<()> {
        let mut choices: Vec<fn(&mut Unstructured, &mut ConfiguredModule<C>) -> Result<Import>> =
            Vec::with_capacity(4);

        if !self.types.is_empty() {
            choices.push(|u, m| {
                let max = m.types.len() as u32 - 1;
                Ok(Import::Func(u.int_in_range(0..=max)?))
            });
        }
        choices.push(|u, m| Ok(Import::Global(m.arbitrary_global_type(u)?)));

        let num_stable_choices = choices.len();

        arbitrary_loop(u, self.config.max_imports(), |u| {
            choices.truncate(num_stable_choices);
            if self.memory_imports() < self.config.max_memories() {
                choices.push(|u, _| Ok(Import::Memory(u.arbitrary()?)));
            }
            if self.table_imports() < self.config.max_tables() {
                choices.push(|u, m| Ok(Import::Table(m.arbitrary_table_type(u)?)));
            }

            let module = limited_string(1_000, u)?;
            let name = limited_string(1_000, u)?;

            let f = u.choose(&choices)?;
            let import = f(u, self)?;
            if let Import::Memory(_) = &import {
                // Remove the memory import choice, since we don't support
                // multiple memories.
                choices.pop();
            }

            self.imports.push((module, name, import));
            Ok(())
        })
    }

    fn funcs<'a>(&'a self) -> impl Iterator<Item = (u32, &'a FuncType)> + 'a {
        self.imports
            .iter()
            .filter_map(|(_, _, imp)| match imp {
                Import::Func(ty) => Some(*ty),
                _ => None,
            })
            .chain(self.funcs.iter().cloned())
            .map(move |ty| &self.types[ty as usize])
            .enumerate()
            .map(|(f, ty)| (f as u32, ty))
    }

    fn get_memory_type(&self, index: u32) -> Option<&MemoryType> {
        let mut i = 0;

        for mem in self.imports.iter().filter_map(|imp| match imp {
            (_, _, Import::Memory(m)) => Some(m),
            _ => None,
        }) {
            if i == index {
                return Some(mem);
            }
            i += 1;
        }

        let index = index - i;
        self.memories.get(index as usize)
    }

    fn func_imports(&self) -> u32 {
        self.imports
            .iter()
            .filter(|imp| matches!(imp, (_, _, Import::Func(_))))
            .count() as u32
    }

    fn table_imports(&self) -> u32 {
        self.imports
            .iter()
            .filter(|imp| matches!(imp, (_, _, Import::Table(_))))
            .count() as u32
    }

    fn memory_imports(&self) -> u32 {
        self.imports
            .iter()
            .filter(|imp| matches!(imp, (_, _, Import::Memory(_))))
            .count() as u32
    }

    fn global_imports(&self) -> u32 {
        self.imports
            .iter()
            .filter(|imp| matches!(imp, (_, _, Import::Global(_))))
            .count() as u32
    }

    fn arbitrary_valtype(&self, u: &mut Unstructured) -> Result<ValType> {
        Ok(*u.choose(&self.valtypes)?)
    }

    fn arbitrary_global_type(&self, u: &mut Unstructured) -> Result<GlobalType> {
        Ok(GlobalType {
            val_type: self.arbitrary_valtype(u)?,
            mutable: u.arbitrary()?,
        })
    }

    fn arbitrary_table_type(&self, u: &mut Unstructured) -> Result<TableType> {
        Ok(TableType {
            elem_ty: if self.config.reference_types_enabled() {
                ValType::FuncRef
            } else {
                *u.choose(&[ValType::FuncRef, ValType::ExternRef])?
            },
            limits: Limits::limited(u, 1_000_000)?,
        })
    }

    fn arbitrary_funcs(&mut self, u: &mut Unstructured) -> Result<()> {
        if self.types.is_empty() {
            return Ok(());
        }

        arbitrary_loop(u, self.config.max_funcs(), |u| {
            let max = self.types.len() as u32 - 1;
            let ty = u.int_in_range(0..=max)?;
            self.funcs.push(ty);
            Ok(())
        })
    }

    fn arbitrary_tables(&mut self, u: &mut Unstructured) -> Result<()> {
        let max_tables = self.config.max_tables() - self.table_imports();
        arbitrary_loop(u, max_tables as usize, |u| {
            let ty = self.arbitrary_table_type(u)?;
            self.tables.push(ty);
            Ok(())
        })
    }

    fn arbitrary_memories(&mut self, u: &mut Unstructured) -> Result<()> {
        let max_mems = self.config.max_memories() - self.memory_imports();
        arbitrary_loop(u, max_mems as usize, |u| {
            self.memories.push(u.arbitrary()?);
            Ok(())
        })
    }

    fn arbitrary_globals(&mut self, u: &mut Unstructured) -> Result<()> {
        let mut choices: Vec<Box<dyn Fn(&mut Unstructured, ValType) -> Result<Instruction>>> =
            vec![];

        arbitrary_loop(u, self.config.max_globals(), |u| {
            let ty = self.arbitrary_global_type(u)?;

            choices.clear();
            let num_funcs = self.funcs.len() as u32;
            choices.push(Box::new(move |u, ty| {
                Ok(match ty {
                    ValType::I32 => Instruction::I32Const(u.arbitrary()?),
                    ValType::I64 => Instruction::I64Const(u.arbitrary()?),
                    ValType::F32 => Instruction::F32Const(u.arbitrary()?),
                    ValType::F64 => Instruction::F64Const(u.arbitrary()?),
                    ValType::ExternRef => Instruction::RefNull(ValType::ExternRef),
                    ValType::FuncRef => {
                        if num_funcs > 0 && u.arbitrary()? {
                            let func = u.int_in_range(0..=num_funcs - 1)?;
                            Instruction::RefFunc(func)
                        } else {
                            Instruction::RefNull(ValType::FuncRef)
                        }
                    }
                })
            }));

            let mut global_idx = 0;
            for (_, _, imp) in &self.imports {
                match imp {
                    Import::Global(g) => {
                        if !g.mutable && g.val_type == ty.val_type {
                            choices
                                .push(Box::new(move |_, _| Ok(Instruction::GlobalGet(global_idx))));
                        }
                        global_idx += 1;
                    }
                    _ => {}
                }
            }

            let f = u.choose(&choices)?;
            let expr = f(u, ty.val_type)?;
            self.globals.push(Global { ty, expr });
            Ok(())
        })
    }

    fn arbitrary_exports(&mut self, u: &mut Unstructured) -> Result<()> {
        let mut choices: Vec<fn(&mut Unstructured, &mut ConfiguredModule<C>) -> Result<Export>> =
            Vec::with_capacity(4);

        if !self.funcs.is_empty() {
            choices.push(|u, m| {
                let max = m.func_imports() + m.funcs.len() as u32 - 1;
                let idx = u.int_in_range(0..=max)?;
                Ok(Export::Func(idx))
            });
        }

        if self.table_imports() > 0 || !self.tables.is_empty() {
            choices.push(|u, m| {
                let max = m.table_imports() + m.tables.len() as u32 - 1;
                let idx = u.int_in_range(0..=max)?;
                Ok(Export::Table(idx))
            });
        }

        if self.memory_imports() > 0 || !self.memories.is_empty() {
            choices.push(|u, m| {
                let max = m.memory_imports() + m.memories.len() as u32 - 1;
                let idx = u.int_in_range(0..=max)?;
                Ok(Export::Memory(idx))
            });
        }

        if !self.globals.is_empty() {
            choices.push(|u, m| {
                let max = m.global_imports() + m.globals.len() as u32 - 1;
                let idx = u.int_in_range(0..=max)?;
                Ok(Export::Global(idx))
            });
        }

        if choices.is_empty() {
            return Ok(());
        }

        let mut export_names = HashSet::new();
        arbitrary_loop(u, self.config.max_exports(), |u| {
            let mut name = limited_string(1_000, u)?;
            while export_names.contains(&name) {
                name.push_str(&format!("{}", export_names.len()));
            }
            export_names.insert(name.clone());

            let f = u.choose(&choices)?;
            let export = f(u, self)?;
            self.exports.push((name, export));
            Ok(())
        })
    }

    fn arbitrary_start(&mut self, u: &mut Unstructured) -> Result<()> {
        let mut choices = Vec::with_capacity(self.func_imports() as usize + self.funcs.len());
        let mut func_index = 0;

        for (_, _, imp) in &self.imports {
            if let Import::Func(ty) = imp {
                let ty = &self.types[*ty as usize];
                if ty.params.is_empty() && ty.results.is_empty() {
                    choices.push(func_index as u32);
                }
                func_index += 1;
            }
        }

        for ty in &self.funcs {
            let ty = &self.types[*ty as usize];
            if ty.params.is_empty() && ty.results.is_empty() {
                choices.push(func_index as u32);
            }
            func_index += 1;
        }

        if !choices.is_empty() && u.arbitrary().unwrap_or(false) {
            let f = *u.choose(&choices)?;
            self.start = Some(f);
        }

        Ok(())
    }

    fn arbitrary_elems(&mut self, u: &mut Unstructured) -> Result<()> {
        let func_max = self.func_imports() + self.funcs.len() as u32;
        let table_tys = self
            .imports
            .iter()
            .filter_map(|(_, _, imp)| match imp {
                Import::Table(t) => Some(t),
                _ => None,
            })
            .chain(&self.tables)
            .map(|t| t.elem_ty)
            .collect::<Vec<_>>();

        // Create a helper closure to choose an arbitrary offset.
        let mut offset_global_choices = vec![];
        let mut global_index = 0;
        for (_, _, imp) in &self.imports {
            if let Import::Global(g) = imp {
                if !g.mutable && g.val_type == ValType::I32 {
                    offset_global_choices.push(global_index);
                }
                global_index += 1;
            }
        }
        let arbitrary_offset = |u: &mut Unstructured| {
            Ok(if !offset_global_choices.is_empty() && u.arbitrary()? {
                let g = u.choose(&offset_global_choices)?;
                Instruction::GlobalGet(*g)
            } else {
                Instruction::I32Const(u.arbitrary()?)
            })
        };

        let mut choices: Vec<Box<dyn Fn(&mut Unstructured) -> Result<(ElementKind, ValType)>>> =
            Vec::new();

        if table_tys.len() > 0 {
            // If we have at least one table, then the MVP encoding is always
            // available so long as it's a funcref table.
            if table_tys[0] == ValType::FuncRef {
                choices.push(Box::new(|u| {
                    Ok((
                        ElementKind::Active {
                            table: None,
                            offset: arbitrary_offset(u)?,
                        },
                        table_tys[0],
                    ))
                }));
            }

            // If we have reference types enabled, then we can initialize any
            // table, and we can also use the alternate encoding to initialize
            // the 0th table.
            if self.config.reference_types_enabled() {
                choices.push(Box::new(|u| {
                    let i = u.int_in_range(0..=table_tys.len() - 1)? as u32;
                    Ok((
                        ElementKind::Active {
                            table: Some(i),
                            offset: arbitrary_offset(u)?,
                        },
                        table_tys[i as usize],
                    ))
                }));
            }
        }

        // Reference types allows us to create passive and declared element
        // segments.
        if self.config.reference_types_enabled() {
            choices.push(Box::new(|_| Ok((ElementKind::Passive, ValType::FuncRef))));
            choices.push(Box::new(|_| Ok((ElementKind::Passive, ValType::ExternRef))));
            choices.push(Box::new(|_| Ok((ElementKind::Declared, ValType::FuncRef))));
            choices.push(Box::new(|_| {
                Ok((ElementKind::Declared, ValType::ExternRef))
            }));
        }

        if choices.is_empty() {
            return Ok(());
        }

        arbitrary_loop(u, self.config.max_element_segments(), |u| {
            // Choose an a
            let (kind, ty) = u.choose(&choices)?(u)?;
            let items = if ty == ValType::ExternRef
                || (self.config.reference_types_enabled() && u.arbitrary()?)
            {
                let mut init = vec![];
                arbitrary_loop(u, self.config.max_elements(), |u| {
                    init.push(
                        if ty == ValType::ExternRef || func_max == 0 || u.arbitrary()? {
                            None
                        } else {
                            Some(u.int_in_range(0..=func_max - 1)?)
                        },
                    );
                    Ok(())
                })?;
                Elements::Expressions(init)
            } else {
                let mut init = vec![];
                if func_max > 0 {
                    arbitrary_loop(u, self.config.max_elements(), |u| {
                        let func_idx = u.int_in_range(0..=func_max - 1)?;
                        init.push(func_idx);
                        Ok(())
                    })?;
                }
                Elements::Functions(init)
            };

            self.elems.push(ElementSegment { kind, ty, items });
            Ok(())
        })
    }

    fn arbitrary_code(&mut self, u: &mut Unstructured, allow_invalid: bool) -> Result<()> {
        self.code.reserve(self.funcs.len());
        let mut allocs = CodeBuilderAllocations::new(self);
        for ty in &self.funcs {
            let ty = &self.types[*ty as usize];
            let body = self.arbitrary_func_body(u, ty, &mut allocs, allow_invalid)?;
            self.code.push(body);
        }
        Ok(())
    }

    fn arbitrary_func_body(
        &self,
        u: &mut Unstructured,
        ty: &FuncType,
        allocs: &mut CodeBuilderAllocations<C>,
        allow_invalid: bool,
    ) -> Result<Code> {
        let locals = self.arbitrary_locals(u)?;
        let builder = allocs.builder(ty, &locals);
        let instructions = if allow_invalid && u.arbitrary().unwrap_or(false) {
            Instructions::Arbitrary(arbitrary_vec_u8(u)?)
        } else {
            Instructions::Generated(builder.arbitrary(u, self)?)
        };

        Ok(Code {
            locals,
            instructions,
        })
    }

    fn arbitrary_locals(&self, u: &mut Unstructured) -> Result<Vec<ValType>> {
        let mut ret = Vec::new();
        arbitrary_loop(u, 100, |u| {
            ret.push(self.arbitrary_valtype(u)?);
            Ok(())
        })?;
        Ok(ret)
    }

    fn arbitrary_data(&mut self, u: &mut Unstructured) -> Result<()> {
        // With bulk-memory we can generate passive data, otherwise if there are
        // no memories we can't generate any data.
        let memories = self.memories.len() as u32 + self.memory_imports();
        if memories == 0 && !self.config.bulk_memory_enabled() {
            return Ok(());
        }

        let mut choices: Vec<Box<dyn Fn(&mut Unstructured) -> Result<Instruction>>> = vec![];

        arbitrary_loop(u, self.config.max_data_segments(), |u| {
            if choices.is_empty() {
                choices.push(Box::new(|u| Ok(Instruction::I32Const(u.arbitrary()?))));

                let mut global_idx = 0;
                for (_, _, imp) in &self.imports {
                    match imp {
                        Import::Global(g) => {
                            if !g.mutable && g.val_type == ValType::I32 {
                                choices.push(Box::new(move |_| {
                                    Ok(Instruction::GlobalGet(global_idx))
                                }));
                            }
                            global_idx += 1;
                        }
                        _ => {}
                    }
                }
            }

            // Passive data can only be generated if bulk memory is enabled.
            // Otherwise if there are no memories we *only* generate passive
            // data. Finally if all conditions are met we use an input byte to
            // determine if it should be passive or active.
            let kind = if self.config.bulk_memory_enabled() && (memories == 0 || u.arbitrary()?) {
                DataSegmentKind::Passive
            } else {
                let f = u.choose(&choices)?;
                let offset = f(u)?;
                let memory_index = u.int_in_range(0..=memories - 1)?;
                DataSegmentKind::Active {
                    offset,
                    memory_index,
                }
            };
            let init = u.arbitrary()?;
            self.data.push(DataSegment { kind, init });
            Ok(())
        })
    }
}

pub(crate) fn arbitrary_loop(
    u: &mut Unstructured,
    max: usize,
    mut f: impl FnMut(&mut Unstructured) -> Result<()>,
) -> Result<()> {
    for _ in 0..max {
        let keep_going = u.arbitrary().unwrap_or(false);
        if !keep_going {
            break;
        }

        f(u)?;
    }

    Ok(())
}

// Mirror what happens in `Arbitrary for String`, but do so with a clamped size.
fn limited_string(max_size: usize, u: &mut Unstructured) -> Result<String> {
    let size = u.arbitrary_len::<u8>()?;
    let size = std::cmp::min(size, max_size);
    match str::from_utf8(&u.peek_bytes(size).unwrap()) {
        Ok(s) => {
            u.get_bytes(size).unwrap();
            Ok(s.into())
        }
        Err(e) => {
            let i = e.valid_up_to();
            let valid = u.get_bytes(i).unwrap();
            let s = unsafe {
                debug_assert!(str::from_utf8(valid).is_ok());
                str::from_utf8_unchecked(valid)
            };
            Ok(s.into())
        }
    }
}

fn arbitrary_vec_u8(u: &mut Unstructured) -> Result<Vec<u8>> {
    let size = u.arbitrary_len::<u8>()?;
    Ok(u.get_bytes(size)?.to_vec())
}
