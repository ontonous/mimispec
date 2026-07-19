desc?? "家庭账本"
rule$ "所有记录先保存到本地"
func$ Record(entry):
    requires$: entry.amount > 0
    ensures$: entry.saved_locally == true
    steps:
        save entry locally
        return entry >>> done
