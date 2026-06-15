use serde::{Deserialize, Serialize};

/// 意图后缀：附加在关键字、标识符或字符串上，表示作者对该节点的锁定与不确定程度。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Commitment {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "?")]
    Question,
    #[serde(rename = "??")]
    QuestionQuestion,
    #[serde(rename = "$")]
    Locked,
    #[serde(rename = "$$")]
    StrongLocked,
    #[serde(rename = "$?")]
    LockedQuestion,
    #[serde(rename = "$$?")]
    StrongLockedQuestion,
    #[serde(rename = "$??")]
    LockedQuestionQuestion,
    #[serde(rename = "$$??")]
    StrongLockedQuestionQuestion,
}

#[allow(dead_code)]
impl Commitment {
    /// 是否处于某种锁定状态（含锁定但存疑）。
    pub fn is_locked(&self) -> bool {
        matches!(
            self,
            Self::Locked
                | Self::StrongLocked
                | Self::LockedQuestion
                | Self::StrongLockedQuestion
                | Self::LockedQuestionQuestion
                | Self::StrongLockedQuestionQuestion
        )
    }

    /// 是否为强锁定。
    pub fn is_strong_locked(&self) -> bool {
        matches!(
            self,
            Self::StrongLocked | Self::StrongLockedQuestion | Self::StrongLockedQuestionQuestion
        )
    }

    /// 是否带不确定标记（作用于节点本身或锁定本身）。
    pub fn has_question(&self) -> bool {
        matches!(
            self,
            Self::Question | Self::LockedQuestion | Self::StrongLockedQuestion
        )
    }

    /// 是否带完全委托标记。
    pub fn has_question_question(&self) -> bool {
        matches!(
            self,
            Self::QuestionQuestion
                | Self::LockedQuestionQuestion
                | Self::StrongLockedQuestionQuestion
        )
    }
}

impl Default for Commitment {
    fn default() -> Self {
        Commitment::None
    }
}

/// 带模糊后缀的标识符（如 `desc?`、`Order?`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ident {
    pub name: String,
    #[serde(default)]
    pub commitment: Commitment,
}

/// 带模糊后缀的字符串字面量（如 `"..."?`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FString {
    pub value: String,
    #[serde(default)]
    pub commitment: Commitment,
}

/// 源文件根节点（v0.3: fragments 而非 modules，含 imports）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct File {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub imports: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<RuleDef>,
    pub fragments: Vec<Fragment>,
}

/// 顶层 Fragment（v0.3 新架构）。任何 Fragment 都可以作为合法顶层存在。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Fragment {
    Module { module: Module },
    TypeDef { typedef: TypeDef },
    Flow { flow: FlowDef },
    Func { func: FuncDef },
    Ui { ui: UiDef },
    Steps { steps: Vec<Step> }, // v0.3 新增：独立 steps 块
    Expr { expr: Expr },        // v0.3 新增：裸表达式
    UiNode { node: UiNode },    // v0.3 新增：裸 UI 节点
    Placeholder,                // v0.3 新增：... 占位符
}

/// 模块或子模块。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Module {
    pub name: Ident,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<Desc>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<RuleDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub math: Option<MathBlock>,
    #[serde(default)]
    pub items: Vec<Fragment>, // v0.3: Vec<Item> → Vec<Fragment>
    #[serde(default)]
    pub keyword_commitment: Commitment,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeDef {
    pub name: Ident,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<Desc>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<RuleDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub math: Option<MathBlock>,
    pub body: TypeBody,
    #[serde(default)]
    pub keyword_commitment: Commitment,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum TypeBody {
    Enum { variants: Vec<Ident> },
    Record { fields: Vec<Field> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Field {
    pub name: Ident,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<RuleDef>,
    pub type_hint: Vec<Atom>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuleDef {
    pub desc: Desc,
    #[serde(default)]
    pub keyword_commitment: Commitment,
    #[serde(default)]
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlowDef {
    pub name: Ident,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<Desc>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<RuleDef>,
    pub entries: Vec<FlowEntry>,
    #[serde(default)]
    pub keyword_commitment: Commitment,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlowEntry {
    pub state: Ident,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<RuleDef>,
    pub arms: Vec<FlowArm>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlowArm {
    pub to: Ident,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires: Option<Condition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<Desc>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<RuleDef>,
    #[serde(default)]
    pub to_keyword_commitment: Commitment,
    #[serde(default)]
    pub requires_keyword_commitment: Commitment,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FuncDef {
    pub name: Ident,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<Desc>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<RuleDef>,
    pub params: Vec<Param>,
    #[serde(default)]
    pub capabilities: Vec<Capability>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires: Option<Condition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ensures: Option<Condition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub math: Option<MathBlock>,
    pub steps: Vec<Step>,
    #[serde(default)]
    pub keyword_commitment: Commitment,
    #[serde(default)]
    pub requires_keyword_commitment: Commitment,
    #[serde(default)]
    pub ensures_keyword_commitment: Commitment,
    #[serde(default)]
    pub with_keyword_commitment: Commitment,
    #[serde(default)]
    pub steps_keyword_commitment: Commitment,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Param {
    pub name: Ident,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub type_hint: Vec<Atom>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Capability {
    pub name: Ident,
    #[serde(default)]
    pub commitment: Commitment,
}

/// `requires` / `ensures` 条件：结构化表达式或自然语言字符串。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Condition {
    Structured { expr: Expr },
    Natural { text: FString },
}

/// 简单表达式 AST（支持比较、逻辑连接）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Expr {
    Ident {
        value: Ident,
    },
    String {
        value: FString,
    },
    Number {
        value: String,
    },
    Bool {
        value: bool,
        #[serde(default)]
        keyword_commitment: Commitment,
    },
    List {
        items: Vec<Expr>,
    },
    Not {
        expr: Box<Expr>,
        #[serde(default)]
        keyword_commitment: Commitment,
    },
    And {
        left: Box<Expr>,
        right: Box<Expr>,
        #[serde(default)]
        keyword_commitment: Commitment,
    },
    Or {
        left: Box<Expr>,
        right: Box<Expr>,
        #[serde(default)]
        keyword_commitment: Commitment,
    },
    In {
        left: Box<Expr>,
        right: Box<Expr>,
        #[serde(default)]
        keyword_commitment: Commitment,
    },
    Compare {
        left: Box<Expr>,
        op: CompareOp,
        right: Box<Expr>,
    },
    Neg {
        expr: Box<Expr>,
    },
    Add {
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Sub {
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Mul {
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Div {
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Pow {
        left: Box<Expr>,
        right: Box<Expr>,
    },
    MatMul {
        left: Box<Expr>,
        right: Box<Expr>,
    },
    BitAnd {
        left: Box<Expr>,
        right: Box<Expr>,
    },
    BitOr {
        left: Box<Expr>,
        right: Box<Expr>,
    },
    BitXor {
        left: Box<Expr>,
        right: Box<Expr>,
    },
    BitNot {
        expr: Box<Expr>,
    },
    Shl {
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Shr {
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Index {
        object: Box<Expr>,
        field: Ident,
    },
    Subscript {
        object: Box<Expr>,
        index: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    Placeholder {
        #[serde(default)]
        keyword_commitment: Commitment,
    },
}

/// math: 块，包含一组数学语句（定义、约束或推导）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MathBlock {
    pub statements: Vec<MathStatement>,
    #[serde(default)]
    pub keyword_commitment: Commitment,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum MathStatement {
    /// 定义/赋值式：target = value
    Define { target: Expr, value: Expr },
    /// 纯表达式语句（约束、等式、推导等）
    Expr { expr: Expr },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompareOp {
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
}

/// 步骤：动作、控制流、错误处理等。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Step {
    Action { step: ActionStep },
    Assign { step: AssignStep },
    If { step: IfStep },
    For { step: ForStep },
    While { step: WhileStep },
    Parasteps { step: ParastepsStep },
    Error { step: ErrorStep },
    Desc { content: Desc }, // v0.3.1 新增：desc 作为独立 step
    Placeholder { keyword_commitment: Commitment }, // v0.3 新增：... 占位符
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionStep {
    #[serde(default)]
    pub label: Vec<Atom>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<Desc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<ToTarget>,
    #[serde(default)]
    pub on_blocks: Vec<OnBlock>,
}

/// 赋值步骤：target = simple_value。
/// `=` 只能出现在动作行，右侧必须是简单值（枚举值、字段值、字面量、列表字面量）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssignStep {
    pub target: Expr,
    pub value: SimpleValue,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<Desc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<ToTarget>,
    #[serde(default)]
    pub on_blocks: Vec<OnBlock>,
}

/// 赋值右侧允许的简单值。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum SimpleValue {
    Ident {
        value: Ident,
    },
    String {
        value: FString,
    },
    Number {
        value: String,
    },
    Bool {
        value: bool,
        #[serde(default)]
        keyword_commitment: Commitment,
    },
    List {
        items: Vec<Vec<Atom>>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IfStep {
    pub cond: Condition,
    pub then_branch: Vec<Step>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub else_branch: Option<Vec<Step>>,
    #[serde(default)]
    pub if_keyword_commitment: Commitment,
    #[serde(default)]
    pub else_keyword_commitment: Commitment,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ForStep {
    pub var: Ident,
    pub iterable: Vec<Atom>,
    pub body: Vec<Step>,
    #[serde(default)]
    pub keyword_commitment: Commitment,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WhileStep {
    pub cond: Condition,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<Desc>,
    pub body: Vec<Step>,
    #[serde(default)]
    pub keyword_commitment: Commitment,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParastepsStep {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<FString>,
    pub steps: Vec<Step>,
    #[serde(default)]
    pub keyword_commitment: Commitment,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorStep {
    pub message: FString,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<ToTarget>,
    #[serde(default)]
    pub keyword_commitment: Commitment,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OnBlock {
    pub condition: Vec<Atom>,
    pub steps: Vec<Step>,
    #[serde(default)]
    pub keyword_commitment: Commitment,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToTarget {
    pub target: Ident,
}

/// `desc` 独立语义：关键字位置 `?` 表示存在性不确定；字符串位置 `?` 表示内容不确定。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Desc {
    #[serde(default)]
    pub need_commitment: Commitment,
    pub content: FString,
}

/// 原始词法单元，用于保留 AI/人类书写的自由动作标签或类型提示。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Atom {
    Ident { value: Ident },
    String { value: FString },
    Number { value: String },
    Symbol { value: String },
    List { items: Vec<Vec<Atom>> },
}

// ── UI 块 ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiDef {
    pub name: Ident,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binds: Option<Ident>,
    pub root: UiNode,
    #[serde(default)]
    pub keyword_commitment: Commitment,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum UiNode {
    Stack { stack: StackNode },
    Parallel { parallel: StackNode },
    Leaf { leaf: UiLeaf },
    Error { error: UiErrorNode },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiErrorNode {
    pub message: FString,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<Desc>,
    #[serde(default)]
    pub keyword_commitment: Commitment,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StackNode {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<FString>,
    pub children: Vec<UiNode>,
    #[serde(default)]
    pub keyword_commitment: Commitment,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiLeaf {
    pub content: FString,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<Desc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires: Option<Condition>,
    #[serde(default)]
    pub with: Vec<Capability>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on: Option<OnBinding>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OnBinding {
    pub event_name: EventName,
    pub action: ActionExpr,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum EventName {
    Ident { value: Ident },
    Natural { text: FString },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionExpr {
    pub actions: Vec<Action>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Action {
    Call { expr: Expr },
    Navigate { target: Ident },
    Assign { target: Expr, value: Expr },
    Natural { text: FString },
}
