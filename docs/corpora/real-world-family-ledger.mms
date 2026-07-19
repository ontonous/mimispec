// MimiSpec 0.3 Core real-world usability corpus — local-first family ledger.
//
// This is intentionally a cohesive product document rather than a collection
// of isolated syntax examples. A product owner should be able to review the
// intent and constraints, while an implementation team can use the structured
// types, Flow, contracts, steps, and UI as a shared delivery boundary.

desc?? "我想做一个家庭共同使用的日常账本，老人也能在几秒内记下一笔开销"
desc$ "第一版只做支出记录、月度汇总和可选的家庭设备同步"

rule$ "任何时候都必须先保存到本地；断网不能阻止记账"
rule$ "金额、备注和家庭成员信息默认不发送给第三方"
rule$ "同一笔记录重复同步不得产生两笔支出"
rule "删除和修改必须保留可追溯的本地历史"
rule?? "家庭成员之间发生编辑冲突时的最终合并策略由 AI 提出候选，再由人确认"

type$ ExpenseCategory:
    desc$ "第一版使用少量、容易理解的固定分类"
    Food | Transport | Housing | Health | Education | Other

type$ Expense:
    desc$ "一笔已经写入本地账本的支出"
    id: Identifier
    amount: Decimal
    category: ExpenseCategory
    occurred_at: Timestamp
    note: Text
    author: HouseholdMember
    sync_state: SyncState

type$ SyncState: LocalOnly | Uploading | Synced | RetryPending

flow$ ExpenseSync:
    LocalOnly:
        on NetworkAvailable >>> Uploading: requires$: expense.saved_locally == true
    Uploading:
        on UploadAccepted >>> Synced: ensures$: remote.copy_count == 1
        on UploadFailed >>> RetryPending: desc$ "失败保持可见，同时保留完整本地记录"
    RetryPending:
        on RetryDue >>> Uploading: requires$: retry.backoff_elapsed == true
        on UserDisablesSync >>> LocalOnly: ensures$: local.expense_preserved == true
    Synced:
        on LocalEdit >>> LocalOnly: desc$ "本地修改产生新版本，再按同一流程同步"

module$ Ledger:
    desc$ "本地账本是支出记录的事实来源；同步服务只是可选副本"

    rule$ "写入月度汇总必须和保存支出属于同一次本地操作"
    func$ RecordExpense(draft):
        desc$ "校验并保存一笔支出；网络状态不参与成功条件"
        requires$: draft.amount > 0
        requires$: draft.category in [Food, Transport, Housing, Health, Education, Other]
        ensures$: expense.saved_locally == true
        ensures$: monthly_summary.includes_expense == true
        steps:
            validate amount and category
            assign stable local identifier
            save expense locally
            update monthly summary
            if sync.enabled and network.online:
                enqueue background upload
            return saved expense >>> done

    rule$ "导入同一文件两次不得重复增加支出"
    func$ ImportExpenses(file):
        desc$ "从家庭成员导出的文件中导入支出"
        requires$: file.format_supported == true
        requires$: file.readable == true
        ensures$: imported.duplicates == 0
        ensures$: imported.invalid_rows_reported == true
        steps:
            read rows
            validate each row
            match stable identifiers
            save new expenses locally
            update monthly summary
            return import report >>> done

    func$ MonthlySummary(month):
        desc$ "按分类展示当月支出，并明确标出仍未同步的记录"
        requires$: month.is_known == true
        ensures$: summary.total == summary.category_totals.sum
        ensures$: summary.unsynced_count >= 0
        steps:
            load local expenses for month
            group by category
            calculate totals
            return summary >>> done

ui$ QuickExpenseEntry binds draft:
    stack "记一笔":
        "金额" desc$ "默认聚焦，支持大字号数字键盘"
        "分类" desc$ "显示六个固定分类，不要求老人搜索"
        "备注" desc "可选，不填写也能保存"
        parallel "操作":
            "保存" desc$ "点击一次立即写入本地" on tap: RecordExpense(draft)
            "取消" on tap: CancelEntry()

ui$ MonthlyOverview binds summary:
    stack "本月支出":
        "@summary.total" desc$ "醒目显示当月总额"
        "@summary.category_totals" desc "按分类展示"
        "@summary.unsynced_count" desc$ "只在存在未同步记录时提示，不阻止继续记账"

rule$ "界面不得用‘同步成功’代替‘本地保存成功’，两者必须分别展示"
