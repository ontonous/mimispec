use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

/// JSON schema carried by every serialized 0.3 semantic document.
pub const AST_SCHEMA_VERSION: &str = "mimispec.ast/0.3";

/// 意图后缀：附加在关键字、标识符或字符串上，表示作者对该节点的锁定与不确定程度。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[non_exhaustive]
pub enum Commitment {
    #[serde(rename = "none")]
    #[default]
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

    /// 是否带不确定标记（单 `?`，作用于节点本身或锁定本身）。
    pub fn has_question(&self) -> bool {
        matches!(
            self,
            Self::Question
                | Self::LockedQuestion
                | Self::StrongLockedQuestion
                | Self::QuestionQuestion
                | Self::LockedQuestionQuestion
                | Self::StrongLockedQuestionQuestion
        )
    }

    /// 是否带完全委托标记（`??`，不含锁定成分）。
    #[allow(dead_code)]
    pub fn has_question_question(&self) -> bool {
        matches!(self, Self::QuestionQuestion)
    }

    /// 将后缀分解为内容保护级别。
    pub fn lock_intent(self) -> LockIntent {
        match self {
            Self::None | Self::Question | Self::QuestionQuestion => LockIntent::Open,
            Self::Locked | Self::LockedQuestion | Self::LockedQuestionQuestion => {
                LockIntent::Locked
            }
            Self::StrongLocked
            | Self::StrongLockedQuestion
            | Self::StrongLockedQuestionQuestion => LockIntent::StrongLocked,
        }
    }

    /// 将后缀分解为审阅或委托意图。
    pub fn review_intent(self) -> ReviewIntent {
        match self {
            Self::None | Self::Locked | Self::StrongLocked => ReviewIntent::None,
            Self::Question | Self::LockedQuestion | Self::StrongLockedQuestion => {
                ReviewIntent::Review
            }
            Self::QuestionQuestion
            | Self::LockedQuestionQuestion
            | Self::StrongLockedQuestionQuestion => ReviewIntent::Delegate,
        }
    }

    /// 返回 `?` / `??` 当前讨论的对象。
    pub fn review_target(self) -> ReviewTarget {
        match self.lock_intent() {
            LockIntent::Open => ReviewTarget::Content,
            LockIntent::Locked => ReviewTarget::Lock,
            LockIntent::StrongLocked => ReviewTarget::StrongLock,
        }
    }

    /// 所有包含 `$` 的状态都保护当前内容。
    pub fn protects_content(self) -> bool {
        self.lock_intent() != LockIntent::Open
    }

    /// 只有没有待审阅问题的锁定状态可进入物化选择。
    pub fn is_commit_ready(self) -> bool {
        matches!(self, Self::Locked | Self::StrongLocked)
    }

    /// 当前意图是否已经由 Human 最终确认。
    ///
    /// 这是 0.3 的规范名称；`is_commit_ready()` 仅作为兼容别名保留。
    pub fn is_confirmed(self) -> bool {
        matches!(self, Self::Locked | Self::StrongLocked)
    }

    /// `??` 表示委托，不论它讨论内容、普通锁还是强锁。
    pub fn is_delegated(self) -> bool {
        self.review_intent() == ReviewIntent::Delegate
    }

    /// 单问号状态需要 Human 作出决定。
    pub fn requires_human_decision(self) -> bool {
        self.review_intent() == ReviewIntent::Review
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LockIntent {
    Open,
    Locked,
    StrongLocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewIntent {
    None,
    Review,
    Delegate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewTarget {
    Content,
    Lock,
    StrongLock,
}

impl std::fmt::Display for Commitment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Commitment::None => "",
            Commitment::Question => "?",
            Commitment::QuestionQuestion => "??",
            Commitment::Locked => "$",
            Commitment::StrongLocked => "$$",
            Commitment::LockedQuestion => "$?",
            Commitment::StrongLockedQuestion => "$$?",
            Commitment::LockedQuestionQuestion => "$??",
            Commitment::StrongLockedQuestionQuestion => "$$??",
        };
        write!(f, "{}", s)
    }
}

/// 带模糊后缀的标识符（如 `desc?`、`Order?`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ident {
    pub name: String,
    #[serde(default)]
    pub commitment: Commitment,
}

/// 带模糊后缀的字符串字面量（如 `"..."?`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FString {
    pub value: String,
    #[serde(default)]
    pub commitment: Commitment,
}

/// 源文件根节点。
///
/// `fragments` 是兼容保留的 Rust 字段名；从 0.3 起它表示 Document Context 内
/// 唯一、跨类型有序的 Context Item 序列，而不只是具名 Fragment。
#[derive(Debug, Clone, PartialEq)]
pub struct File {
    pub imports: Vec<String>,
    pub fragments: Vec<Fragment>,
}

impl Serialize for File {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state =
            serializer.serialize_struct("File", if self.imports.is_empty() { 2 } else { 3 })?;
        state.serialize_field("schema_version", AST_SCHEMA_VERSION)?;
        if !self.imports.is_empty() {
            state.serialize_field("imports", &self.imports)?;
        }
        state.serialize_field("items", &self.fragments)?;
        state.end()
    }
}

/// 0.3 Context Item / Fragment。
///
/// 同一个 enum 同时用于 Document、module、func、type、flow 和 steps 的有序
/// body；各 parser 只接受对应 scope 合法的 variant。`ContextItem` 是它的公开
/// 语义别名。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind")]
#[non_exhaustive]
pub enum Fragment {
    Desc {
        desc: Desc,
    },
    Rule {
        rule: RuleDef,
    },
    Clause {
        clause: Clause,
    },
    Module {
        module: Module,
    },
    TypeDef {
        typedef: TypeDef,
    },
    Flow {
        flow: FlowDef,
    },
    Func {
        func: FuncDef,
    },
    Ui {
        ui: UiDef,
    },
    Steps {
        #[serde(default)]
        keyword_commitment: Commitment,
        items: Vec<Fragment>,
    },
    Step {
        step: Step,
    },
    Expr {
        expr: Expr,
    },
    UiNode {
        node: UiNode,
    },
    Math {
        math: MathBlock,
    },
    Field {
        field: Field,
    },
    Variants {
        variants: Vec<Ident>,
    },
    FlowEntry {
        entry: FlowEntry,
    },
    FlowArm {
        arm: FlowArm,
    },
    Placeholder {
        #[serde(default)]
        keyword_commitment: Commitment,
    },
}

pub type ContextItem = Fragment;

pub fn rules_attached_to(items: &[Fragment], target_index: usize) -> Vec<&RuleDef> {
    items
        .iter()
        .filter_map(Fragment::rule)
        .filter(|rule| {
            matches!(
                rule.attachment,
                RuleAttachment::Attached {
                    target_index: target
                } if target == target_index
            )
        })
        .collect()
}

impl File {
    pub fn rules(&self) -> Vec<&RuleDef> {
        self.fragments.iter().filter_map(Fragment::rule).collect()
    }

    pub fn environment_rules(&self) -> Vec<&RuleDef> {
        self.rules()
            .into_iter()
            .filter(|rule| rule.attachment == RuleAttachment::Environment)
            .collect()
    }
}

/// 模块或子模块。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Module {
    pub name: Ident,
    #[serde(default)]
    pub items: Vec<Fragment>,
    #[serde(default)]
    pub keyword_commitment: Commitment,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TypeDef {
    pub name: Ident,
    pub body: TypeBody,
    #[serde(default)]
    pub keyword_commitment: Commitment,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind")]
#[non_exhaustive]
pub enum TypeBody {
    Enum {
        #[serde(default)]
        inline: bool,
        items: Vec<Fragment>,
    },
    Record {
        items: Vec<Fragment>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Field {
    pub name: Ident,
    pub type_hint: Vec<Atom>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RuleDef {
    pub desc: Desc,
    #[serde(default)]
    pub keyword_commitment: Commitment,
    pub attachment: RuleAttachment,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RuleAttachment {
    #[default]
    Pending,
    Attached {
        target_index: usize,
    },
    Environment,
    UnresolvedByRecovery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClauseKind {
    Requires,
    Ensures,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Clause {
    pub clause_kind: ClauseKind,
    pub condition: Condition,
    #[serde(default)]
    pub keyword_commitment: Commitment,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FlowDef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<Ident>,
    pub items: Vec<Fragment>,
    #[serde(default)]
    pub keyword_commitment: Commitment,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FlowEntry {
    pub state: Ident,
    pub items: Vec<Fragment>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FlowArm {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<FlowEvent>,
    pub to: Ident,
    pub items: Vec<Fragment>,
    #[serde(default)]
    pub to_keyword_commitment: Commitment,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FlowEvent {
    #[serde(default)]
    pub keyword_commitment: Commitment,
    pub name: EventName,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FuncDef {
    pub name: Ident,
    pub params: Vec<Param>,
    #[serde(default)]
    pub capabilities: Vec<Capability>,
    pub items: Vec<Fragment>,
    #[serde(default)]
    pub keyword_commitment: Commitment,
    #[serde(default)]
    pub with_keyword_commitment: Commitment,
}

impl Fragment {
    pub fn rule(&self) -> Option<&RuleDef> {
        match self {
            Self::Rule { rule } => Some(rule),
            _ => None,
        }
    }

    pub fn rule_mut(&mut self) -> Option<&mut RuleDef> {
        match self {
            Self::Rule { rule } => Some(rule),
            _ => None,
        }
    }

    pub fn desc(&self) -> Option<&Desc> {
        match self {
            Self::Desc { desc } => Some(desc),
            _ => None,
        }
    }

    pub fn clause(&self) -> Option<&Clause> {
        match self {
            Self::Clause { clause } => Some(clause),
            _ => None,
        }
    }
}

impl Module {
    pub fn descs(&self) -> impl Iterator<Item = &Desc> {
        self.items.iter().filter_map(Fragment::desc)
    }

    pub fn rules(&self) -> Vec<&RuleDef> {
        self.items.iter().filter_map(Fragment::rule).collect()
    }

    pub fn math_blocks(&self) -> Vec<&MathBlock> {
        self.items
            .iter()
            .filter_map(|item| match item {
                Fragment::Math { math } => Some(math),
                _ => None,
            })
            .collect()
    }
}

impl TypeDef {
    pub fn items(&self) -> &[Fragment] {
        match &self.body {
            TypeBody::Enum { items, .. } | TypeBody::Record { items } => items,
        }
    }

    pub fn items_mut(&mut self) -> &mut Vec<Fragment> {
        match &mut self.body {
            TypeBody::Enum { items, .. } | TypeBody::Record { items } => items,
        }
    }

    pub fn descs(&self) -> impl Iterator<Item = &Desc> {
        self.items().iter().filter_map(Fragment::desc)
    }

    pub fn rules(&self) -> Vec<&RuleDef> {
        self.items().iter().filter_map(Fragment::rule).collect()
    }

    pub fn fields(&self) -> Vec<&Field> {
        self.items()
            .iter()
            .filter_map(|item| match item {
                Fragment::Field { field } => Some(field),
                _ => None,
            })
            .collect()
    }

    pub fn variants(&self) -> Vec<&Ident> {
        self.items()
            .iter()
            .flat_map(|item| match item {
                Fragment::Variants { variants } => variants.as_slice(),
                _ => &[],
            })
            .collect()
    }
}

impl FuncDef {
    pub fn descs(&self) -> impl Iterator<Item = &Desc> {
        self.items.iter().filter_map(Fragment::desc)
    }

    pub fn rules(&self) -> Vec<&RuleDef> {
        self.items.iter().filter_map(Fragment::rule).collect()
    }

    pub fn clauses(&self) -> Vec<&Clause> {
        self.items.iter().filter_map(Fragment::clause).collect()
    }

    pub fn requires(&self) -> Vec<&Clause> {
        self.clauses()
            .into_iter()
            .filter(|clause| clause.clause_kind == ClauseKind::Requires)
            .collect()
    }

    pub fn ensures(&self) -> Vec<&Clause> {
        self.clauses()
            .into_iter()
            .filter(|clause| clause.clause_kind == ClauseKind::Ensures)
            .collect()
    }

    pub fn step_refs(&self) -> Vec<&Step> {
        let mut out = Vec::new();
        collect_step_refs(&self.items, &mut out);
        out
    }

    pub fn steps(&self) -> Vec<&Step> {
        self.step_refs()
    }

    pub fn math_blocks(&self) -> Vec<&MathBlock> {
        self.items
            .iter()
            .filter_map(|item| match item {
                Fragment::Math { math } => Some(math),
                _ => None,
            })
            .collect()
    }

    pub fn has_math(&self) -> bool {
        self.items
            .iter()
            .any(|item| matches!(item, Fragment::Math { .. }))
    }
}

fn collect_step_refs<'a>(items: &'a [Fragment], out: &mut Vec<&'a Step>) {
    for item in items {
        match item {
            Fragment::Step { step } => out.push(step),
            Fragment::Steps { items, .. } => collect_step_refs(items, out),
            _ => {}
        }
    }
}

impl FlowDef {
    pub fn entries(&self) -> Vec<&FlowEntry> {
        self.items
            .iter()
            .filter_map(|item| match item {
                Fragment::FlowEntry { entry } => Some(entry),
                _ => None,
            })
            .collect()
    }

    pub fn rules(&self) -> Vec<&RuleDef> {
        self.items.iter().filter_map(Fragment::rule).collect()
    }
}

impl FlowEntry {
    pub fn arms(&self) -> Vec<&FlowArm> {
        self.items
            .iter()
            .filter_map(|item| match item {
                Fragment::FlowArm { arm } => Some(arm),
                _ => None,
            })
            .collect()
    }

    pub fn rules(&self) -> Vec<&RuleDef> {
        self.items.iter().filter_map(Fragment::rule).collect()
    }
}

impl FlowArm {
    pub fn clauses(&self) -> Vec<&Clause> {
        self.items.iter().filter_map(Fragment::clause).collect()
    }

    pub fn descs(&self) -> Vec<&Desc> {
        self.items.iter().filter_map(Fragment::desc).collect()
    }

    pub fn rules(&self) -> Vec<&RuleDef> {
        self.items.iter().filter_map(Fragment::rule).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Param {
    pub name: Ident,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub type_hint: Vec<Atom>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Capability {
    pub name: Ident,
    #[serde(default)]
    pub commitment: Commitment,
}

/// `requires` / `ensures` 条件：结构化表达式或自然语言字符串。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind")]
#[non_exhaustive]
pub enum Condition {
    Structured { expr: Expr },
    Natural { text: FString },
}

/// 简单表达式 AST（支持比较、逻辑连接）。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind")]
#[non_exhaustive]
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
        #[serde(default)]
        keyword_commitment: Commitment,
    },
    Neg {
        expr: Box<Expr>,
        #[serde(default)]
        keyword_commitment: Commitment,
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
        #[serde(default)]
        keyword_commitment: Commitment,
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
        indices: Vec<Expr>,
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
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MathBlock {
    pub statements: Vec<MathStatement>,
    #[serde(default)]
    pub keyword_commitment: Commitment,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind")]
#[non_exhaustive]
pub enum MathStatement {
    /// 定义/赋值式：target = value
    Define { target: Expr, value: Expr },
    /// 纯表达式语句（约束、等式、推导等）
    Expr { expr: Expr },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub enum CompareOp {
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
}

/// 步骤：动作、控制流、错误处理等。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind")]
#[non_exhaustive]
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

#[derive(Debug, Clone, PartialEq, Serialize)]
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
#[derive(Debug, Clone, PartialEq, Serialize)]
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
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind")]
#[non_exhaustive]
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
    Placeholder {
        commitment: Commitment,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IfStep {
    pub cond: Condition,
    pub then_branch: Vec<Fragment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub else_branch: Option<Vec<Fragment>>,
    #[serde(default)]
    pub if_keyword_commitment: Commitment,
    #[serde(default)]
    pub else_keyword_commitment: Commitment,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ForStep {
    pub var: Ident,
    pub iterable: Vec<Atom>,
    pub body: Vec<Fragment>,
    #[serde(default)]
    pub keyword_commitment: Commitment,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WhileStep {
    pub cond: Condition,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<Desc>,
    pub body: Vec<Fragment>,
    #[serde(default)]
    pub keyword_commitment: Commitment,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ParastepsStep {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<FString>,
    pub steps: Vec<Fragment>,
    #[serde(default)]
    pub keyword_commitment: Commitment,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ErrorStep {
    pub message: FString,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<ToTarget>,
    #[serde(default)]
    pub keyword_commitment: Commitment,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OnBlock {
    pub condition: Vec<Atom>,
    pub steps: Vec<Fragment>,
    #[serde(default)]
    pub keyword_commitment: Commitment,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ToTarget {
    pub target: Ident,
}

/// `desc` 独立语义：关键字位置 `?` 表示存在性不确定；字符串位置 `?` 表示内容不确定。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Desc {
    #[serde(default)]
    pub need_commitment: Commitment,
    pub content: FString,
}

/// 原始词法单元，用于保留 AI/人类书写的自由动作标签或类型提示。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind")]
#[non_exhaustive]
pub enum Atom {
    Ident { value: Ident },
    String { value: FString },
    Number { value: String },
    Symbol { value: String },
    List { items: Vec<Vec<Atom>> },
    Ellipsis { commitment: Commitment },
}

// ── UI 块 ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UiDef {
    pub name: Ident,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binds: Option<Ident>,
    #[serde(default)]
    pub binds_keyword_commitment: Commitment,
    pub items: Vec<Fragment>,
    #[serde(default)]
    pub keyword_commitment: Commitment,
}

impl UiDef {
    pub fn rules(&self) -> Vec<&RuleDef> {
        self.items.iter().filter_map(Fragment::rule).collect()
    }

    pub fn root(&self) -> Option<&UiNode> {
        self.items.iter().find_map(|item| match item {
            Fragment::UiNode { node } => Some(node),
            _ => None,
        })
    }

    pub fn is_placeholder(&self) -> bool {
        matches!(self.items.as_slice(), [Fragment::Placeholder { .. }])
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind")]
#[non_exhaustive]
pub enum UiNode {
    Stack { stack: StackNode },
    Parallel { parallel: StackNode },
    Leaf { leaf: UiLeaf },
    Error { error: UiErrorNode },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UiErrorNode {
    pub message: FString,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<Desc>,
    #[serde(default)]
    pub keyword_commitment: Commitment,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StackNode {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<FString>,
    pub items: Vec<Fragment>,
    #[serde(default)]
    pub keyword_commitment: Commitment,
}

impl StackNode {
    pub fn children(&self) -> Vec<&UiNode> {
        self.items
            .iter()
            .filter_map(|item| match item {
                Fragment::UiNode { node } => Some(node),
                _ => None,
            })
            .collect()
    }

    pub fn rules(&self) -> Vec<&RuleDef> {
        self.items.iter().filter_map(Fragment::rule).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UiLeaf {
    pub content: FString,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<Desc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires: Option<Condition>,
    #[serde(default)]
    pub requires_keyword_commitment: Commitment,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub with: Vec<Capability>,
    #[serde(default)]
    pub with_keyword_commitment: Commitment,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on: Option<OnBinding>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OnBinding {
    pub event_name: EventName,
    pub action: ActionExpr,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind")]
#[non_exhaustive]
pub enum EventName {
    Ident { value: Ident },
    Natural { text: FString },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ActionExpr {
    pub actions: Vec<Action>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind")]
#[non_exhaustive]
pub enum Action {
    Call { expr: Expr },
    Navigate { target: Ident },
    Assign { target: Expr, value: Expr },
    Natural { text: FString },
}
