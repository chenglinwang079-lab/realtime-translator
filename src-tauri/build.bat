@echo off
call "E:\BuildTools\VC\Auxiliary\Build\vcvarsall.bat" x64
cargo check
