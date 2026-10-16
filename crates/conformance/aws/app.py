"""Credential-free, asset-free synthesis; Rust alone submits the resulting template."""
import argparse
import json
from pathlib import Path
from aws_cdk import App
from stack import RunnerStack


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    request = json.loads(args.input.read_text())
    app = App(outdir=str(args.out), analytics_reporting=False, auto_synth=False)
    stack = RunnerStack(app, request)
    assembly = app.synth()
    template = assembly.get_stack_by_name(stack.stack_name).template
    print(json.dumps(template, sort_keys=True, separators=(",", ":")))


if __name__ == "__main__":
    main()
