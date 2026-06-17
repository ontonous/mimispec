// FILE: types.mms
// MimiSpec 解析器实现描述 — 核心类型

module CoreTypes:

    desc "AST 和词法单元的核心类型定义"

    // === TokenKind 枚举（34 个关键字 + 24 个符号 + 5 个布局 token） ===

    type KeywordToken:
        | module | type | rule | flow | func | ui | parallel
        | stack | binds | requires | ensures | steps | math
        | parasteps | if | else | for | while | desc | on
        | with | error | and | or | not | in | done | exit
        | true | false | import | ellipsis | at

    type PunctuationToken:
        | colon | comma | pipe | lparen | rparen
        | lbracket | rbracket | assign | dot | eqeq | noteq
        | lt | gt | le | ge

    type MathOpToken:
        | plus | minus | star | slash | power
        | bitand | bitxor | bitnot | shl | shr | arrow

    type FuzzyToken: question | questionquestion | dollar | dollardollar

    type LayoutToken: indent | dedent | newline | eof

    type LiteralToken: ident | string | number

    type Token:
        kind: TokenKind
        line: usize
        col: usize

    // === Commitment 后缀 ===

    type Commitment:
        | none | question | questionquestion | locked
        | stronglocked | lockedquestion | stronglockedquestion
        | lockedquestionquestion | stronglockedquestionquestion

    type Ident:
        name: string
        commitment: Commitment

    type FString:
        value: string
        commitment: Commitment

    // === Fragment ===

    type Fragment:
        | fg_module | fg_typedef | fg_flow | fg_func
        | fg_ui | fg_steps | fg_expr | fg_uinode | fg_placeholder

    type File:
        imports: string
        rules: RuleDef
        fragments: Fragment

    type RuleDef:
        rule_desc: Desc
        keywordcommitment: Commitment

    type Desc:
        needcommitment: Commitment
        content: FString

    type TypeBody:
        | tb_enum | tb_record

    type Condition:
        | cond_structured | cond_natural

    type Step:
        | st_action | st_assign | st_if | st_for
        | st_while | st_parasteps | st_error | st_desc | st_placeholder

    type Expr:
        | expr_ident | expr_string | expr_number | expr_bool
        | expr_list | expr_not | expr_and | expr_or | expr_in
        | expr_compare | expr_neg | expr_add | expr_sub
        | expr_mul | expr_div | expr_pow | expr_matmul
        | expr_bitand | expr_bitor | expr_bitxor | expr_bitnot
        | expr_shl | expr_shr | expr_index | expr_subscript
        | expr_call | expr_placeholder

    type CompareOp: eq | ne | lt | gt | le | ge

    type UiNode:
        | ui_stack | ui_parallel | ui_leaf | ui_error

    type Atom:
        | atom_ident | atom_string | atom_number | atom_symbol | atom_list

    type SimpleValue:
        | sv_ident | sv_string | sv_number | sv_bool | sv_list
