"""Synthesize the separately deployed AG4 shared stack without AWS access."""
import argparse
import json
from pathlib import Path

from aws_cdk import App
from shared_stack import SharedStack


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    config = json.loads(args.input.read_text())
    runner = json.loads((Path(__file__).parent / "runner.json").read_text())
    app = App(outdir=str(args.out), analytics_reporting=False, auto_synth=False)
    stack = SharedStack(app, config, runner["image"])
    assembly = app.synth()
    print(json.dumps(assembly.get_stack_by_name(stack.stack_name).template,
                     sort_keys=True, separators=(",", ":")))


if __name__ == "__main__":
    main()
