// Cargo.toml (for reference)
// [package]
// name = "resumable_vm"
// version = "0.1.0"
// edition = "2021"
// [dependencies]
// serde = { version = "1", features = ["derive"] }
// serde_json = "1"
// thiserror = "1"

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/* ===================== Values ===================== */

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "t", content = "v")]
pub enum Val {
    Unit,
    Bool(bool),
    Num(i64),
    Str(String),
    List(Vec<Val>),
    Obj(HashMap<String, Val>),
}

/* ===================== Language (subset) ===================== */

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Stmt {
    Block { body: Vec<Stmt> },
    ExprStmt { expr: Expr },
    Let { name: String, init: Option<Expr> },
    Assign { name: String, expr: Expr },
    If { test: Expr, then_s: Box<Stmt>, else_s: Option<Box<Stmt>> },
    While { test: Expr, body: Box<Stmt> },
    Break,
    Continue,
    Return { value: Option<Expr> },
    Try { try_s: Box<Stmt>, catch_name: Option<String>, catch_s: Option<Box<Stmt>>, finally_s: Option<Box<Stmt>> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Expr {
    LitBool { v: bool },
    LitNum { v: i64 },
    LitStr { v: String },
    Ident { name: String },
    Call  { callee: Box<Expr>, args: Vec<Expr> }, // sync only in v1
    Await { inner: Box<Expr>, policy: AwaitPolicy }, // only allowed in ExprStmt/Assign
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AwaitPolicy { Any, All }

/* ===================== Environment (flat slots) ===================== */

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VMEnv {
    pub slots: Vec<Val>,
    pub sp: usize,
    // temp per-scope name->slot maps until parser assigns indices
    pub names: Vec<HashMap<String, usize>>,
}
impl VMEnv {
    pub fn new() -> Self { Self { slots: vec![], sp: 0, names: vec![HashMap::new()] } }
    pub fn push_scope(&mut self) { self.names.push(HashMap::new()); }
    pub fn pop_scope(&mut self) { self.names.pop(); }
    pub fn truncate(&mut self, base: usize) { self.slots.truncate(base); self.sp = base; }
    pub fn declare(&mut self, name: &str, init: Val) -> usize {
        let idx = self.sp; self.sp += 1; self.slots.push(init);
        if let Some(m) = self.names.last_mut() { m.insert(name.to_string(), idx); }
        idx
    }
    pub fn resolve(&self, name: &str) -> Option<usize> {
        for m in self.names.iter().rev() {
            if let Some(&i) = m.get(name) { return Some(i); }
        }
        None
    }
}

/* ===================== Control & Await Capsule ===================== */

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "k", content = "v")]
pub enum Control {
    None,
    Break,
    Continue,
    Return(Option<Val>),
    Throw(Val),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwaitCapsule {
    pub policy: AwaitPolicy,
    pub task_ids: Vec<String>, // UUIDs
    pub created: bool,         // idempotency guard
    // optional: timestamps/metadata
}

/* ===================== PCs (program counters) ===================== */

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[repr(u8)]
pub enum BlockPc { Enter = 0, Next = 1 }

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[repr(u8)]
pub enum IfPc { EvalCond = 0, Dispatch = 1 }

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[repr(u8)]
pub enum WhilePc { Check = 0, RunBody = 1, PostBody = 2 }

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[repr(u8)]
pub enum TryPc { EnterTry = 0, AfterTry = 1, RunCatch = 2, RunFinally = 3, Done = 4 }

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[repr(u8)]
pub enum ExprStmtPc {
    Done = 0,
    AwaitCreate = 1,
    AwaitWaiting = 2,
    AwaitDone = 3,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[repr(u8)]
pub enum AssignPc {
    Simple = 0,
    AwaitCreate = 1,
    AwaitWaiting = 2,
    AwaitAssign = 3,
    Done = 4,
}

/* ===================== Frames ===================== */

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum FrameKind {
    Block { pc: BlockPc, idx: usize },
    If { pc: IfPc, branch_then: bool },
    While { pc: WhilePc },
    Let { name: String, has_init: bool, done: bool },
    Assign { pc: AssignPc },
    ExprStmt { pc: ExprStmtPc },
    Try { pc: TryPc, has_catch: bool, has_finally: bool, catch_name: Option<String> },
    Break,
    Continue,
    Return,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frame {
    #[serde(flatten)]
    pub kind: FrameKind,
    pub scope_base_sp: usize,
    pub node: Stmt,                 // (v1: clone; can switch to node-id later)
    pub await_value: Option<Val>,   // scratch for await
}

/* ===================== VM ===================== */

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VM {
    pub frames: Vec<Frame>,
    pub env: VMEnv,
    pub control: Control,
    pub await_capsule: Option<AwaitCapsule>, // Some when paused at await
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step { Continue, Yield, Done }

impl VM {
    pub fn new(program: Stmt) -> Self {
        let mut vm = VM { frames: vec![], env: VMEnv::new(), control: Control::None, await_capsule: None };
        push_stmt(&mut vm, &Stmt::Block { body: vec![program] });
        vm
    }
}

/* ===================== Expression evaluator (stub) ===================== */
// v1: expressions are synchronous; await is only at statement level.
fn eval_expr(e: &Expr, env: &mut VMEnv) -> Val {
    match e {
        Expr::LitBool { v } => Val::Bool(*v),
        Expr::LitNum { v } => Val::Num(*v),
        Expr::LitStr { v } => Val::Str(v.clone()),
        Expr::Ident { name } => {
            let idx = env.resolve(name).unwrap_or_else(|| panic!("undefined {name}"));
            env.slots[idx].clone()
        }
        Expr::Call { callee, args } => {
            let _cv = eval_expr(callee, env);
            let _argv: Vec<Val> = args.iter().map(|a| eval_expr(a, env)).collect();
            // TODO: host-provided call hook goes here; for now return Unit
            Val::Unit
        }
        Expr::Await { .. } => unreachable!("await is handled at statement level"),
    }
}

/* ===================== Host hooks (to implement externally) ===================== */

fn create_tasks_for(_expr: &Expr, _env: &mut VMEnv) -> Vec<String> {
    // TODO: Build task(s) from expression; the only allowed side-effect in sandbox.
    vec![format!("task-{}", 1)]
}

fn poll_tasks(_cap: &AwaitCapsule) -> Option<Result<Val, Val>> {
    // TODO: Query task table: None (pending), Some(Ok(val)) done, Some(Err(err)) failed
    Some(Ok(Val::Str("result".into())))
}

/* ===================== Driver ===================== */

pub fn run_for(vm: &mut VM, budget: usize) -> Step {
    for _ in 0..budget {
        match step(vm) {
            Step::Continue => continue,
            other => return other,
        }
    }
    Step::Yield
}

fn step(vm: &mut VM) -> Step {
    // centralized control handling
    if vm.control != Control::None {
        return if unwind(vm) { Step::Continue } else { Step::Done };
    }

    let Some(ix) = vm.frames.len().checked_sub(1) else { return Step::Done; };
    let (kind, node, base) = {
        let f = &vm.frames[ix];
        (f.kind.clone(), f.node.clone(), f.scope_base_sp)
    };

    use FrameKind as FK;
    use Step::*;

    match (kind, node) {
        /* ------- Block ------- */
        (FK::Block { pc, mut idx }, Stmt::Block { body }) => {
            match pc {
                BlockPc::Enter => {
                    vm.env.push_scope();
                    vm.frames[ix].kind = FK::Block { pc: BlockPc::Next, idx };
                    Continue
                }
                BlockPc::Next => {
                    if idx < body.len() {
                        vm.frames[ix].kind = FK::Block { pc: BlockPc::Next, idx: idx + 1 };
                        push_stmt(vm, &body[idx]);
                        Yield
                    } else {
                        vm.env.truncate(base);
                        vm.env.pop_scope();
                        vm.frames.pop();
                        Continue
                    }
                }
            }
        }

        /* ------- Let ------- */
        (FK::Let { name, has_init, done }, Stmt::Let { name: n, init }) => {
            if !done {
                let slot = vm.env.declare(&name, Val::Unit);
                if has_init {
                    if let Some(e) = init {
                        let v = eval_expr(&e, &mut vm.env);
                        vm.env.slots[slot] = v;
                    }
                }
                vm.frames[ix].kind = FK::Let { name: n, has_init, done: true };
                Continue
            } else {
                vm.frames.pop();
                Continue
            }
        }

        /* ------- Assign ------- */
        (FK::Assign { pc }, Stmt::Assign { name, expr }) => match pc {
            AssignPc::Simple => {
                let v = eval_expr(&expr, &mut vm.env);
                let idx = vm.env.resolve(&name).expect("unresolved");
                vm.env.slots[idx] = v;
                vm.frames[ix].kind = FK::Assign { pc: AssignPc::Done };
                Continue
            }
            AssignPc::AwaitCreate => {
                let Expr::Await { inner, policy } = expr else { unreachable!() };
                let task_ids = create_tasks_for(&inner, &mut vm.env);
                vm.await_capsule = Some(AwaitCapsule { policy, task_ids, created: true });
                vm.frames[ix].kind = FK::Assign { pc: AssignPc::AwaitWaiting };
                Yield // suspension point
            }
            AssignPc::AwaitWaiting => {
                if let Some(outcome) = vm.await_capsule.as_ref().and_then(|cap| poll_tasks(cap)) {
                    match outcome {
                        Ok(val) => {
                            vm.frames[ix].await_value = Some(val);
                            vm.await_capsule = None;
                            vm.frames[ix].kind = FK::Assign { pc: AssignPc::AwaitAssign };
                            Continue
                        }
                        Err(err) => {
                            vm.await_capsule = None;
                            vm.control = Control::Throw(err);
                            Continue
                        }
                    }
                } else {
                    Yield
                }
            }
            AssignPc::AwaitAssign => {
                let idx = vm.env.resolve(&name).expect("unresolved");
                let v = vm.frames[ix].await_value.take().expect("await value missing");
                vm.env.slots[idx] = v;
                vm.frames[ix].kind = FK::Assign { pc: AssignPc::Done };
                Continue
            }
            AssignPc::Done => { vm.frames.pop(); Continue }
        },

        /* ------- ExprStmt ------- */
        (FK::ExprStmt { pc }, Stmt::ExprStmt { expr }) => match pc {
            ExprStmtPc::Done => { vm.frames.pop(); Continue }
            ExprStmtPc::AwaitCreate => {
                let Expr::Await { inner, policy } = expr else { unreachable!() };
                let task_ids = create_tasks_for(&inner, &mut vm.env);
                vm.await_capsule = Some(AwaitCapsule { policy, task_ids, created: true });
                vm.frames[ix].kind = FK::ExprStmt { pc: ExprStmtPc::AwaitWaiting };
                Yield
            }
            ExprStmtPc::AwaitWaiting => {
                if let Some(outcome) = vm.await_capsule.as_ref().and_then(|cap| poll_tasks(cap)) {
                    match outcome {
                        Ok(_val) => {
                            vm.await_capsule = None;
                            vm.frames[ix].kind = FK::ExprStmt { pc: ExprStmtPc::AwaitDone };
                            Continue
                        }
                        Err(err) => {
                            vm.await_capsule = None;
                            vm.control = Control::Throw(err);
                            Continue
                        }
                    }
                } else {
                    Yield
                }
            }
            ExprStmtPc::AwaitDone => { vm.frames.pop(); Continue }
        },

        /* ------- If ------- */
        (FK::If { pc, mut branch_then }, Stmt::If { test, then_s, else_s }) => match pc {
            IfPc::EvalCond => {
                let cond = matches!(eval_expr(&test, &mut vm.env), Val::Bool(true));
                branch_then = cond;
                vm.frames[ix].kind = FK::If { pc: IfPc::Dispatch, branch_then };
                Continue
            }
            IfPc::Dispatch => {
                vm.frames.pop();
                let next = if branch_then { *then_s } else { else_s.map(|b| *b).unwrap_or(Stmt::Block { body: vec![] }) };
                push_stmt(vm, &next);
                Yield
            }
        },

        /* ------- While ------- */
        (FK::While { pc }, Stmt::While { test, body }) => match pc {
            WhilePc::Check => {
                let cond = matches!(eval_expr(&test, &mut vm.env), Val::Bool(true));
                if !cond { vm.frames.pop(); Continue } else {
                    vm.control = Control::None;
                    vm.frames[ix].kind = FK::While { pc: WhilePc::RunBody };
                    Continue
                }
            }
            WhilePc::RunBody => {
                vm.frames[ix].kind = FK::While { pc: WhilePc::PostBody };
                push_stmt(vm, &*body);
                Yield
            }
            WhilePc::PostBody => {
                match std::mem::take(&mut vm.control) {
                    Control::Break => { vm.frames.pop(); Continue }
                    Control::Continue => { vm.frames[ix].kind = FK::While { pc: WhilePc::Check }; Continue }
                    Control::Return(v) => { vm.control = Control::Return(v); vm.frames.pop(); Continue }
                    Control::Throw(e)  => { vm.control = Control::Throw(e);  vm.frames.pop(); Continue }
                    Control::None      => { vm.frames[ix].kind = FK::While { pc: WhilePc::Check }; Continue }
                }
            }
        },

        /* ------- Break / Continue / Return ------- */
        (FK::Break, Stmt::Break) => { vm.control = Control::Break; vm.frames.pop(); Continue }
        (FK::Continue, Stmt::Continue) => { vm.control = Control::Continue; vm.frames.pop(); Continue }
        (FK::Return, Stmt::Return { value }) => {
            let v = value.map(|e| eval_expr(&e, &mut vm.env));
            vm.control = Control::Return(v);
            vm.frames.pop();
            Continue
        }

        /* ------- Try/Catch/Finally ------- */
        (FK::Try { pc, has_catch, has_finally, catch_name: _ }, Stmt::Try { try_s, catch_s, finally_s, .. }) => match pc {
            TryPc::EnterTry => {
                vm.frames[ix].kind = FK::Try { pc: TryPc::AfterTry, has_catch, has_finally, catch_name: None };
                push_stmt(vm, &*try_s);
                Yield
            }
            TryPc::AfterTry => {
                match vm.control.clone() {
                    Control::Throw(_) => {
                        if has_finally {
                            vm.frames[ix].kind = FK::Try { pc: TryPc::RunFinally, has_catch, has_finally, catch_name: None };
                            push_stmt(vm, finally_s.as_deref().expect("missing finally"));
                            Yield
                        } else if has_catch {
                            vm.control = Control::None;
                            vm.frames[ix].kind = FK::Try { pc: TryPc::RunCatch, has_catch, has_finally, catch_name: None };
                            push_stmt(vm, catch_s.as_deref().expect("missing catch"));
                            Yield
                        } else {
                            vm.frames.pop(); // keep Throw, outer will handle
                            Continue
                        }
                    }
                    _ => {
                        if has_finally {
                            vm.frames[ix].kind = FK::Try { pc: TryPc::RunFinally, has_catch, has_finally, catch_name: None };
                            push_stmt(vm, finally_s.as_deref().expect("missing finally"));
                            Yield
                        } else {
                            vm.frames.pop();
                            Continue
                        }
                    }
                }
            }
            TryPc::RunCatch => {
                vm.control = Control::None; // handled
                vm.frames.pop();
                Continue
            }
            TryPc::RunFinally => {
                vm.frames.pop();
                Continue
            }
            TryPc::Done => { vm.frames.pop(); Continue }
        },

        /* ------- Safety ------- */
        (other, _n) => panic!("Unhandled frame/node pair: {:?}", other),
    }
}

fn push_stmt(vm: &mut VM, s: &Stmt) {
    use FrameKind as FK;
    let base = vm.env.sp;
    let kind = match s {
        Stmt::Block { .. } => FK::Block { pc: BlockPc::Enter, idx: 0 },
        Stmt::ExprStmt { expr } => match expr {
            Expr::Await { .. } => FK::ExprStmt { pc: ExprStmtPc::AwaitCreate },
            _ => FK::ExprStmt { pc: ExprStmtPc::Done }, // sync expr; single-step
        },
        Stmt::Let { name, init } => FK::Let { name: name.clone(), has_init: init.is_some(), done: false },
        Stmt::Assign { expr, .. } => match expr {
            Expr::Await { .. } => FK::Assign { pc: AssignPc::AwaitCreate },
            _ => FK::Assign { pc: AssignPc::Simple },
        },
        Stmt::If { .. }    => FK::If { pc: IfPc::EvalCond, branch_then: false },
        Stmt::While { .. } => FK::While { pc: WhilePc::Check },
        Stmt::Break        => FK::Break,
        Stmt::Continue     => FK::Continue,
        Stmt::Return { .. }=> FK::Return,
        Stmt::Try { catch_s, finally_s, .. } =>
            FK::Try { pc: TryPc::EnterTry, has_catch: catch_s.is_some(), has_finally: finally_s.is_some(), catch_name: None },
    };
    vm.frames.push(Frame { kind, scope_base_sp: base, node: s.clone(), await_value: None });
}

/* ===================== Unwinding ===================== */

fn unwind(vm: &mut VM) -> bool {
    use Control::*;
    while let Some(f) = vm.frames.pop() {
        vm.env.truncate(f.scope_base_sp);

        match (&vm.control, &f.kind) {
            (Break, FrameKind::While { .. })    => { vm.control = Control::None; return true; }
            (Continue, FrameKind::While { .. }) => { vm.control = Control::Continue; vm.frames.push(f); return true; }
            (Return(_), _) => continue, // bubble to outer
            (Throw(_), FrameKind::Try { .. }) => { vm.frames.push(f); return true; } // Try handler will run
            (Throw(_), _) => continue, // keep popping
            (None, _) => {}
        }
    }
    // If Throw falls off the bottom => uncaught; Done. If Return falls off => program return.
    !matches!(vm.control, Control::Throw(_))
}

/* ===================== Demo (optional) ===================== */

#[cfg(test)]
mod demo {
    use super::*;

    #[test]
    fn simple_await_assign_roundtrip() {
        let prog = Stmt::Assign {
            name: "r".into(),
            expr: Expr::Await { inner: Box::new(Expr::LitStr { v: "input".into() }), policy: AwaitPolicy::All },
        };
        let mut vm = VM::new(Stmt::Block { body: vec![Stmt::Let { name: "r".into(), init: None }, prog] });

        // Run until it suspends at await
        let s1 = run_for(&mut vm, 100);
        assert_eq!(s1, Step::Yield);
        assert!(vm.await_capsule.is_some());

        // Persist / restore
        let json = serde_json::to_string(&vm).unwrap();
        let mut vm2: VM = serde_json::from_str(&json).unwrap();

        // Resume; poll_tasks returns ready immediately in this stub
        loop {
            match run_for(&mut vm2, 100) {
                Step::Done => break,
                Step::Yield | Step::Continue => continue,
            }
        }
        // Completed: r should be set (Unit in stub or "result" depending on poll_tasks impl)
        let idx = vm2.env.resolve("r").unwrap();
        let _r = vm2.env.slots[idx].clone();
    }
}
