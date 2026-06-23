// FILE: types.mms
// MimiSpec 解析器参考实现 — 核心 AST 类型定义

module CoreTypes:

    desc "AST 和词法单元的核心类型定义，对应 src/lib/ast.rs"

    // === TokenKind 枚举 ===

    type TokenKind:
        | kw_module | kw_type | kw_rule | kw_flow | kw_func | kw_ui
        | kw_parallel | kw_stack | kw_binds | kw_requires | kw_ensures
        | kw_steps | kw_math | kw_parasteps | kw_if | kw_else | kw_for
        | kw_while | kw_desc | kw_on | kw_with | kw_error | kw_and
        | kw_or | kw_not | kw_in | kw_done | kw_exit | kw_true
        | kw_false | kw_import | kw_ellipsis | kw_at
        | lit_ident | lit_string | lit_number
        | colon | comma | pipe | lparen | rparen
        | lbracket | rbracket | assign | dot
        | eqeq | noteq | lt | gt | le | ge
        | plus | minus | star | slash | power
        | bitand | bitxor | bitnot | shl | shr
        | arrow
        | question | questionquestion | dollar | dollardollar
        | indent | dedent | newline | eof

    type Token:
        kind: TokenKind
        line: usize
        col: usize

    // === Commitment 后缀 ===

    type Commitment:
        | none | question | questionquestion
        | locked | stronglocked
        | lockedquestion | lockedquestionquestion
        | stronglockedquestion | stronglockedquestionquestion

    type Ident:
        name: string
        commitment: Commitment

    type FString:
        value: string
        commitment: Commitment

    // === Fragment 体系 ===

    type Fragment:
        | fg_module | fg_typedef | fg_flow | fg_func
        | fg_ui | fg_steps | fg_expr | fg_uinode | fg_placeholder

    type File:
        imports: string
        global_rules: RuleDef
        fragments: Fragment

    // === Module ===

    type Module:
        name: Ident
        math_block: MathBlock
        module_items: Fragment
        keyword_commitment: Commitment

    // === Type 定义 ===

    type TypeBody:
        | tb_enum | tb_record

    type TypeDef:
        name: Ident
        typedef_body: TypeBody
        keyword_commitment: Commitment

    type Field:
        field_name: Ident
        field_rules: RuleDef
        type_hint: Atom

    // === Rule ===

    type RuleDef:
        rule_desc: Desc
        keyword_commitment: Commitment
        line: usize

    type Desc:
        need_commitment: Commitment
        content: FString

    // === Flow ===

    type FlowDef:
        name: Ident
        entries: FlowEntry
        keyword_commitment: Commitment

    type FlowEntry:
        state: Ident
        arms: FlowArm

    type FlowArm:
        target: Ident
        arm_requires: Condition
        arm_desc: Desc
        to_keyword_commitment: Commitment
        requires_keyword_commitment: Commitment

    // === Func ===

    type FuncDef:
        name: Ident
        params: Param
        capabilities: Capability
        pre_condition: Condition
        post_condition: Condition
        math_block: MathBlock
        func_steps: Step
        keyword_commitment: Commitment
        requires_keyword_commitment: Commitment
        ensures_keyword_commitment: Commitment
        with_keyword_commitment: Commitment
        steps_keyword_commitment: Commitment

    type Param:
        name: Ident
        type_hint: Atom

    type Capability:
        name: Ident
        commitment: Commitment

    // === Condition ===

    type Condition:
        | cond_structured | cond_natural

    // === Step ===

    type Step:
        | st_action | st_assign | st_if | st_for
        | st_while | st_parasteps | st_error | st_desc | st_placeholder

    type ActionStep:
        label: Atom
        step_to: ToTarget
        on_blocks: OnBlock

    type AssignStep:
        assign_target: Expr
        value: SimpleValue
        assign_to: ToTarget
        on_blocks: OnBlock

    type IfStep:
        cond: Condition
        then_branch: Step
        else_branch: Step
        if_keyword_commitment: Commitment
        else_keyword_commitment: Commitment

    type ForStep:
        loop_var: Ident
        iterable: Atom
        body: Step
        keyword_commitment: Commitment

    type WhileStep:
        cond: Condition
        body: Step
        keyword_commitment: Commitment

    type ParastepsStep:
        sub_steps: Step
        keyword_commitment: Commitment

    type ErrorStep:
        message: FString
        err_to: ToTarget
        keyword_commitment: Commitment

    type OnBlock:
        on_condition: Atom
        on_steps: Step
        keyword_commitment: Commitment

    type ToTarget:
        target: Ident

    // === Expression ===

    type Expr:
        | expr_ident | expr_string | expr_number | expr_bool
        | expr_list | expr_not | expr_and | expr_or | expr_in
        | expr_compare | expr_neg | expr_add | expr_sub
        | expr_mul | expr_div | expr_pow | expr_matmul
        | expr_bitand | expr_bitor | expr_bitxor | expr_bitnot
        | expr_shl | expr_shr | expr_index | expr_subscript
        | expr_call | expr_placeholder

    type CompareOp: eq | ne | lt | gt | le | ge

    type MathBlock:
        statements: MathStatement
        keyword_commitment: Commitment

    type MathStatement:
        | ms_define | ms_expr

    // === SimpleValue ===

    type SimpleValue:
        | sv_ident | sv_string | sv_number | sv_bool | sv_list

    // === Atom ===

    type Atom:
        | atom_ident | atom_string | atom_number | atom_symbol | atom_list

    // === UI ===

    type UiDef:
        name: Ident
        binds: Ident
        root: UiNode
        keyword_commitment: Commitment

    type UiNode:
        | ui_stack | ui_parallel | ui_leaf | ui_error

    type StackNode:
        children: UiNode
        keyword_commitment: Commitment

    type UiErrorNode:
        message: FString
        keyword_commitment: Commitment

    type UiLeaf:
        content: FString
        leaf_requires: Condition
        leaf_with: Capability
        on_binding: OnBinding

    type OnBinding:
        event_name: EventName
        action: ActionExpr

    type EventName:
        | ev_ident | ev_natural

    type ActionExpr:
        actions: Action

    type Action:
        | act_call | act_navigate | act_assign | act_natural
