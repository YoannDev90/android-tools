# Shared SHA-256 helper.
function Get-Sha256File($path) {
    return (Get-FileHash -Path $path -Algorithm SHA256).Hash.ToLower()
}
