"""Offline child probe; never reads or changes the operator's AWS home.

Invoked through the production Rust command owner with AwsEnvironment applied.
The optional vendored Botocore mode is library evidence, not CLI qualification.
"""
import importlib
import json
import os
from pathlib import Path
import pwd
import sys
from types import SimpleNamespace

home = Path(os.environ["HOME"])
assert home.is_absolute() and home.is_dir()
assert home.stat().st_mode & 0o777 == 0o700
assert not list(home.iterdir()), list(home.rglob("*"))
fake_account_home = Path(sys.argv[1])
external = fake_account_home / ".aws/models/ag4external/2026-01-01/service-2.json"
external.parent.mkdir(parents=True)
external.write_text(json.dumps({"version": "2.0", "metadata": {
    "apiVersion": "2026-01-01", "endpointPrefix": "sts", "protocol": "query",
    "serviceId": "AG4External", "signatureVersion": "v4"},
    "operations": {"Probe": {"name": "Probe", "http": {"method": "POST", "requestUri": "/"},
                              "input": {"shape": "Input"}}},
    "shapes": {"Input": {"type": "structure", "members": {"ExternalMarker": {"shape": "Text"}}},
               "Text": {"type": "string"}}}))
# Confine password-database substitution to this test process, before importing
# the library (its default customer path is computed during import).
pwd.getpwuid = lambda _uid: SimpleNamespace(pw_dir=str(fake_account_home))
del os.environ["HOME"]
assert Path(os.path.expanduser("~/.aws/models")) == external.parents[2]
if len(sys.argv) > 2:
    loaders = importlib.import_module("awscli.botocore.loaders")
    assert loaders.Loader().load_service_model("ag4external", "service-2")["metadata"]["serviceId"] == "AG4External"
os.environ["HOME"] = str(home)
assert not Path(os.path.expanduser("~/.aws/models")).exists()
assert Path(os.path.expanduser("~")) == home
if len(sys.argv) > 2:
    loaders = importlib.reload(loaders)
    assert "ag4external" not in loaders.Loader().list_available_services("service-2")
    assert loaders.Loader().load_service_model("sts", "service-2")["metadata"]["endpointPrefix"] == "sts"
assert not list(home.iterdir()), list(home.rglob("*"))
print("private home blocks password-home model discovery; " + ("vendored Botocore library" if len(sys.argv) > 2 else "stdlib filesystem helper"))
