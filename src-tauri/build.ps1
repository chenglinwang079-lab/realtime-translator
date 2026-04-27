& 'E:\BuildTools\VC\Auxiliary\Build\vcvarsall.bat' x64
$cargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
& $cargo check
