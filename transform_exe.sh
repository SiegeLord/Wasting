#!/bin/bash
set -e
cp target/release/Wasting.exe .
magick convert raw/icon.png -define icon:auto-resize:256,128,96,64,48,32,16 -compress zip raw/icon.ico
winpty ../ResourceHacker/ResourceHacker.exe -open Wasting.exe -save Wasting.exe -action addoverwrite -res raw/icon.ico -mask ICONGROUP,MAINICON,
