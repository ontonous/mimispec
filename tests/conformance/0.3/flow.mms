flow$ Sync:
    LocalOnly:
        on UploadRequested >>> Uploading: requires$: local.saved == true
    Uploading:
        on UploadFailed >>> RetryPending: ensures$: local.preserved == true
        on UploadAccepted >>> Synced:
