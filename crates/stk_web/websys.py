#!/usr/bin/env python3

# adds / removes websys features

import tomllib
import argparse
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument("op", choices=["add", "rm"])
parser.add_argument("features", nargs="+")
args = parser.parse_args()

with open("Cargo.toml", "rb") as f:
    current = tomllib.load(f)

current = current["dependencies"]["web-sys"]["features"]

match args.op:
    case "add":
        for feature in args.features:
            current.append(feature)
    case "rm":
        for feature in args.features:
            current.remove(feature)
    case _:
        print("unknown subcommand")
        exit()

current = list(dict.fromkeys(current)) # deduplicate
command = f"cargo add web-sys --features={','.join(current)}"
print(command)

res = subprocess.run(command, shell=True, capture_output=True)

if res.returncode != 0:
    print("failed. is feature name correct?")
    exit()

with open("websys-features", "wb") as f:
    f.write(res.stderr)
print("done")
print("if you're with rust-analyzer, reload Cargo.toml to take effect!")
