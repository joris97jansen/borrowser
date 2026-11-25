"""AJ5 supervisor tests use owned tiny processes, never engine timing thresholds."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location("aj5_probe", Path(__file__).resolve().parents[2] / "perf/aj5_diagnostics_probe.py")
probe = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probe)
INPUTS = {(n, k): hashlib.sha256(probe.input_bytes(n, k)).hexdigest() for n, k in {c[:2] for c in probe.CASES}}


def lines(cases=None):
    return "".join(f"AJ5_CASE_START {n} {k} {w} 1:1 {INPUTS[n,k]}\nAJ5_CASE_END {n} {k} {w} 1 {INPUTS[n,k]}\n"
                   for n, k, w in (cases if cases is not None else sorted(probe.CASES)))


class Aj5ProbeTests(unittest.TestCase):
    def child(self, source, **limits):
        return probe.supervise([sys.executable, "-c", source], inputs=INPUTS, **limits)

    def assert_clean(self, result):
        self.assertTrue(result["cleanup"]["verified_absent"], result)
        self.assertIsNone(result["cleanup"]["failure"])
        self.assertFalse(probe.group_exists(result["cleanup"]["pgid"]))

    def test_complete_eighteen_cases_requires_verified_group_cleanup(self):
        result = self.child(f"print({lines()!r},end='')")
        self.assertIsNone(result["failure"])
        self.assertEqual((len(result["rows"]),len(result["records"])), (18,36))
        self.assert_clean(result)

    def descendant(self, *, retain_pipes):
        with tempfile.TemporaryDirectory() as directory:
            pidfile = Path(directory)/"descendant"
            stopfile = Path(directory)/"stop"
            # The unrelated sibling must survive our group cleanup.
            sibling = subprocess.Popen([sys.executable,"-c","import time;time.sleep(30)"],start_new_session=True)
            try:
                child = f"import pathlib,time;end=time.monotonic()+30\nwhile time.monotonic()<end and not pathlib.Path({str(stopfile)!r}).exists():time.sleep(.005)"
                extra = "" if retain_pipes else ",stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL"
                source = f"import subprocess,sys,pathlib;d=subprocess.Popen([sys.executable,'-c',{child!r}]{extra});pathlib.Path({str(pidfile)!r}).write_text(str(d.pid));print({lines()!r},end='')"
                result = self.child(source,process_seconds=.3,case_seconds=.3)
                self.assert_clean(result)
                self.assertIsNone(sibling.poll(), "unrelated process must survive")
                pid = int(pidfile.read_text())
                with self.assertRaises(ProcessLookupError):
                    os.kill(pid,0)
                return result
            finally:
                stopfile.touch()
                sibling.kill(); sibling.wait(timeout=10)
                if pidfile.exists():
                    deadline=time.monotonic()+10
                    while True:
                        try:os.kill(int(pidfile.read_text()),0)
                        except ProcessLookupError:break
                        if time.monotonic()>=deadline:raise AssertionError("fixture descendant cleanup unresolved")
                        time.sleep(.01)

    def test_successful_leader_with_redirected_descendant_is_actually_cleaned(self):
        result = self.descendant(retain_pipes=False)
        self.assertIsNone(result["failure"])
        self.assertEqual(result["exit"],0)
        self.assertEqual(len(result["rows"]),18)

    def test_descendant_retaining_pipe_times_out_and_is_actually_cleaned(self):
        result = self.descendant(retain_pipes=True)
        self.assertEqual(result["failure"],"timeout")

    def test_cleanup_failure_is_not_measurement_success(self):
        real_cleanup = probe.cleanup_group
        def unresolved(child):
            status, cleanup = real_cleanup(child)
            self.assertTrue(cleanup["verified_absent"])
            cleanup.update(verified_absent=False,failure="injected unresolved group verification")
            return status, cleanup
        with mock.patch.object(probe,"cleanup_group",side_effect=unresolved):
            result=self.child(f"print({lines()!r},end='')")
        self.assertEqual(len(result["rows"]),18)
        self.assertEqual(result["failure"],"cleanup-unverified")

    def test_group_verification_error_is_fail_closed(self):
        real_exists=probe.group_exists
        with mock.patch.object(probe,"group_exists",side_effect=PermissionError("verification refused")):
            result=self.child(f"print({lines()!r},end='')")
        self.assertEqual(result["failure"],"cleanup-unverified")
        self.assertIn("verification refused",result["cleanup"]["failure"])
        self.assertFalse(real_exists(result["cleanup"]["pgid"]))

    def test_nonzero_missing_start_duplicate_and_malformed_are_failures(self):
        one=lines([sorted(probe.CASES)[0]])
        for source in ["raise SystemExit(3)","pass",f"print({one+one!r},end='')","print('unexpected')",f"print({one.splitlines()[1]!r})"]:
            result=self.child(source)
            self.assertIsNotNone(result["failure"])
            self.assert_clean(result)

    def test_missing_end_and_bad_sha_or_configuration_are_failures(self):
        first=lines().splitlines()[0]
        for output in [first,lines().replace(INPUTS[65536,'clean'],'0'*64,1),lines().replace('1:1','0:1',1)]:
            result=self.child(f"print({output!r})")
            self.assertIsNotNone(result["failure"])
            self.assert_clean(result)

    def test_timeout_reaps_and_verifies_group(self):
        result=self.child("import time;time.sleep(30)",process_seconds=.1,case_seconds=.1)
        self.assertEqual(result["failure"],"timeout")
        self.assert_clean(result)
        self.assertLess(result["seconds"],10)

    def test_output_overflow_is_failure_and_bounded(self):
        result=self.child("import sys;sys.stderr.write('x'*10000)",output_limit=128)
        self.assertEqual(result["failure"],"output-limit")
        self.assertLessEqual(len(result["stderr"]),128)
        self.assert_clean(result)

    def test_signal_interrupt_reaps_and_verifies_group(self):
        for sig in ("SIGTERM","SIGINT"):
            result=self.child(f"import os,signal,time;time.sleep(.05);os.kill(os.getppid(),signal.{sig});time.sleep(30)")
            self.assertEqual(result["failure"],f"signal {getattr(signal,sig)}")
            self.assert_clean(result)

    def test_setup_failure_and_interruption_cannot_leak_child(self):
        for action in [OSError("setup refused"),lambda *_:os.kill(os.getpid(),signal.SIGTERM)]:
            with mock.patch.object(probe.os,"set_blocking",side_effect=action):
                result=self.child("import time;time.sleep(30)")
            self.assertIsNotNone(result["failure"])
            self.assert_clean(result)

    def test_interrupt_during_spawn_is_deferred_until_child_is_owned(self):
        original=probe.subprocess.Popen
        def spawn(*args,**kwargs):
            child=original(*args,**kwargs);os.kill(os.getpid(),signal.SIGTERM);return child
        with mock.patch.object(probe.subprocess,"Popen",side_effect=spawn):
            result=self.child("import time;time.sleep(30)")
        self.assertIsNotNone(result["failure"])
        self.assert_clean(result)

    def test_explicit_progress_is_bounded_to_36_start_end_records(self):
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/"progress.jsonl"
            result=self.child(f"print({lines()!r},end='')",progress_path=path)
            self.assertIsNone(result["failure"])
            self.assertEqual(len(path.read_text().splitlines()),36)
            self.assertLessEqual(path.stat().st_size,8192)

    def campaign(self,directory):
        root=Path(directory)
        for n,k in {c[:2]for c in probe.CASES}:
            (root/probe.input_name(n,k)).write_bytes(probe.input_bytes(n,k))
        manifest=root/'inputs.sha256';manifest.write_text(''.join(f"{value}  {probe.input_name(n,k)}\n"for(n,k),value in sorted(INPUTS.items())))
        args=argparse.Namespace(evidence=root/'evidence',input_directory=root,input_manifest=manifest,warmups=1,samples=1,
                               baseline_source='base',candidate_source='candidate',probe_source=root/'probe',baseline=root/'base',candidate=root/'candidate')
        for name in ['baseline','candidate']:
            receipt=root/(name+'.json');receipt.write_text(json.dumps(dict(cwd=str(root),source_fingerprint=getattr(args,name+'_source'),executable_sha256=name,probe_sha256='probe',exit=0,source_unchanged=True,manifest_sha256='manifest',lock_sha256='lock')))
            setattr(args,name+'_receipt',receipt)
        identity=dict(executables=dict(baseline='baseline',candidate='candidate'),probe_sha256='probe',sources=dict(baseline='base',candidate='candidate'),build_inputs={n:dict(manifest_sha256='manifest',lock_sha256='lock')for n in ['baseline','candidate']})
        result=dict(failure=None,exit=0,cleanup=dict(verified_absent=True),rows=[(*c,1)for c in probe.CASES],records=[None]*36)
        return args,identity,result

    def test_campaign_accepts_exactly_180_rows_five_alternating_pairs(self):
        with tempfile.TemporaryDirectory()as d:
            args,identity,result=self.campaign(d)
            with mock.patch.object(probe,'identities',return_value=identity),mock.patch.object(probe,'supervise',side_effect=lambda *a,**k:dict(result))as run:
                final=probe.run_campaign(args)
            self.assertTrue(final['complete']);self.assertEqual(final['accepted_measurements'],180)
            names=[Path(c.args[0][0]).name for c in run.call_args_list]
            self.assertEqual(names,['base','candidate','candidate','base','base','candidate','candidate','base','base','candidate'])

    def test_incomplete_or_unresolved_cleanup_stops_campaign_without_next_process(self):
        for bad in ['measurement','cleanup','count']:
            with tempfile.TemporaryDirectory()as d:
                args,identity,result=self.campaign(d)
                if bad=='measurement':result['failure']='timeout'
                elif bad=='cleanup':result['cleanup']=dict(verified_absent=False)
                else:result['rows']=result['rows'][:-1]
                with mock.patch.object(probe,'identities',return_value=identity),mock.patch.object(probe,'supervise',return_value=result)as run:
                    with self.assertRaisesRegex(ValueError,'incomplete'):probe.run_campaign(args)
                self.assertEqual(run.call_count,1)
                final=json.loads((args.evidence/'campaign.json').read_text());self.assertFalse(final['complete']);self.assertEqual(final['accepted_measurements'],0)

    def test_campaign_identity_drift_stops_before_another_process(self):
        with tempfile.TemporaryDirectory()as d:
            args,identity,result=self.campaign(d)
            with mock.patch.object(probe,'identities',side_effect=[identity,identity,dict(identity,probe_sha256='changed')]),mock.patch.object(probe,'supervise',return_value=result)as run:
                with self.assertRaisesRegex(ValueError,'incomplete'):probe.run_campaign(args)
            self.assertEqual(run.call_count,1)

    def test_external_manifest_binds_all_six_shared_files(self):
        with tempfile.TemporaryDirectory()as d:
            args,_,_=self.campaign(d)
            self.assertEqual(probe.load_inputs(args.input_directory,args.input_manifest),INPUTS)
            (args.input_directory/probe.input_name(65536,"clean")).write_bytes(b"x"*65536)
            with self.assertRaisesRegex(ValueError,"manifest mismatch"):probe.load_inputs(args.input_directory,args.input_manifest)

    def test_unapproved_warmup_or_sample_configuration_never_spawns(self):
        for field in ['warmups','samples']:
            with tempfile.TemporaryDirectory()as d:
                args,_,_=self.campaign(d);setattr(args,field,2)
                with mock.patch.object(probe,'supervise')as run:
                    with self.assertRaisesRegex(ValueError,'exactly one'):probe.run_campaign(args)
                run.assert_not_called()

    def test_actual_signal_refusal_leaves_failure_until_fixture_recovers(self):
        with tempfile.TemporaryDirectory()as d:
            stop=Path(d)/"stop";pidfile=Path(d)/"pid"
            child=f"import pathlib,time;end=time.monotonic()+30\nwhile time.monotonic()<end and not pathlib.Path({str(stop)!r}).exists():time.sleep(.005)"
            source=f"import subprocess,sys,pathlib;p=subprocess.Popen([sys.executable,'-c',{child!r}],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL);pathlib.Path({str(pidfile)!r}).write_text(str(p.pid));print({lines()!r},end='')"
            original=probe.os.killpg
            def refuse_group(pgid,sig):
                if sig==signal.SIGKILL:raise PermissionError('injected owned-group signal refusal')
                return original(pgid,sig)
            try:
                with mock.patch.object(probe.os,'killpg',side_effect=refuse_group):result=self.child(source)
                self.assertEqual(result['failure'],'cleanup-unverified')
                self.assertFalse(result['cleanup']['verified_absent'])
                os.kill(int(pidfile.read_text()),0)  # Actual survivor, not just mocked status.
            finally:
                stop.touch();deadline=time.monotonic()+10
                while pidfile.exists():
                    try:os.kill(int(pidfile.read_text()),0)
                    except ProcessLookupError:break
                    if time.monotonic()>=deadline:raise AssertionError('test-owned recovery unresolved')
                    time.sleep(.01)

    def smoke_fixture(self,directory):
        root=Path(directory);exe=root/'smoke';exe.write_bytes(b'AJ5 fixture executable')
        def result(argv,**_):
            mode=argv[1]if len(argv)>1 else ''
            return dict(exit=(-6 if mode=='abort' else 1 if mode else 0),
                        failure='exit-status'if mode else None,cleanup=dict(verified_absent=True),
                        stdout='AJ5_SMOKE_SERVER 127.0.0.1:12345\nAJ5_SMOKE_RESULT primary='+('Err("injected loopback failure")'if mode else 'Ok(())')+' server=Ok(()) runtime=Ok(()) cleanup_verified=true\n',
                        stderr='AJ5 controlled optimized-abort fixture'if mode=='abort'else'')
        return root,exe,result

    def test_loopback_qualification_requires_all_success_and_failure_cleanup_cases(self):
        with tempfile.TemporaryDirectory()as d:
            root,exe,result=self.smoke_fixture(d)
            with mock.patch.object(probe,'supervise',side_effect=result)as run,mock.patch.object(probe.socket,'create_connection',side_effect=ConnectionRefusedError):
                probe.qualify_smoke(exe,root/'evidence')
            self.assertEqual(run.call_count,5)
            for receipt in (root/'evidence').glob('*.json'):self.assertTrue(json.loads(receipt.read_text())['qualification_passed'])

    def test_expected_nonzero_does_not_mask_timeout_or_unresolved_smoke_cleanup(self):
        for defect in ['timeout','cleanup']:
            with tempfile.TemporaryDirectory()as d:
                root,exe,valid=self.smoke_fixture(d)
                def bad(argv,**kwargs):
                    result=valid(argv,**kwargs)
                    if len(argv)>1:
                        if defect=='timeout':result['failure']='timeout'
                        else:result['cleanup']=dict(verified_absent=False)
                    return result
                with mock.patch.object(probe,'supervise',side_effect=bad)as run,mock.patch.object(probe.socket,'create_connection',side_effect=ConnectionRefusedError):
                    with self.assertRaisesRegex(ValueError,'incomplete'):probe.qualify_smoke(exe,root/'evidence')
                self.assertEqual(run.call_count,2)

    def test_interrupt_after_leader_exit_reaps_descendant_with_retained_pipes(self):
        # Preserve the original interruption-after-exit case, now with actual
        # group and descendant absence checks, rather than leader status alone.
        for sig in ('SIGINT','SIGTERM'):
            with tempfile.TemporaryDirectory()as d:
                stop=Path(d)/'stop';pidfile=Path(d)/'pid'
                child=("import os,signal,time,pathlib\nlimit=time.monotonic()+2\n"
                       "while os.getppid()=={leader} and time.monotonic()<limit:time.sleep(.001)\n"
                       "if os.getppid()=={leader}:raise SystemExit(4)\n"
                       "os.kill({owner},signal."+sig+")\nlimit=time.monotonic()+30\n"
                       "while not pathlib.Path("+repr(str(stop))+").exists() and time.monotonic()<limit:time.sleep(.005)")
                source=("import os,subprocess,sys,pathlib;code="+repr(child)+".format(leader=os.getpid(),owner=os.getppid());"
                        "p=subprocess.Popen([sys.executable,'-c',code]);pathlib.Path("+repr(str(pidfile))+").write_text(str(p.pid))")
                try:
                    result=self.child(source,process_seconds=3,case_seconds=3)
                    self.assertEqual(result['failure'],f"signal {getattr(signal,sig)}")
                    self.assert_clean(result)
                    with self.assertRaises(ProcessLookupError):os.kill(int(pidfile.read_text()),0)
                finally:
                    stop.touch();deadline=time.monotonic()+10
                    while pidfile.exists():
                        try:os.kill(int(pidfile.read_text()),0)
                        except ProcessLookupError:break
                        if time.monotonic()>=deadline:raise AssertionError('retained-pipe fixture recovery unresolved')
                        time.sleep(.01)
