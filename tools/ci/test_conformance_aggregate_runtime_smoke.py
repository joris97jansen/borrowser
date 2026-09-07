"""Independent strace records for the AG9e command-specific isolation audit."""
import unittest
from conformance_aggregate_runtime_smoke import verify_trace

BINARY = b'/tmp/runner\xff'
INITIAL = '42 execve("' + ''.join(f'\\x{byte:02x}' for byte in BINARY) + '", [], []) = 0\n'


class TraceContract(unittest.TestCase):
    def test_exact_initial_launch(self):
        verify_trace(INITIAL + '42 +++ exited with 0 +++\n', BINARY)

    def test_threads_and_unfinished_thread_calls_are_allowed(self):
        for record in [
            '42 clone(child_stack=NULL, flags=CLONE_VM|CLONE_THREAD|CLONE_SIGHAND) = 43\n',
            '42 clone3({flags=CLONE_VM|CLONE_THREAD|CLONE_SIGHAND, exit_signal=0}, 88) = 43\n',
            '42 clone(child_stack=NULL, flags=CLONE_VM|CLONE_THREAD <unfinished ...>\n43 exit(0) = ?\n42 <... clone resumed>) = 43\n',
        ]:
            verify_trace(INITIAL + record, BINARY)

    def test_fork_like_or_unclassified_creation_fails(self):
        for record in ['fork() = 43', 'vfork() = 43',
                       'clone(child_stack=NULL, flags=CLONE_VM|SIGCHLD) = 43',
                       'clone3({flags=0, exit_signal=SIGCHLD}, 88) = 43',
                       'clone3(0x1234, 88) = -1 EFAULT']:
            with self.subTest(record=record), self.assertRaises(RuntimeError):
                verify_trace(INITIAL + record, BINARY)

    def test_additional_exec_even_from_thread_fails(self):
        for syscall in ['execve("browser", [], []) = 0', 'execveat(3, "", [], [], AT_EMPTY_PATH) = 0']:
            with self.assertRaises(RuntimeError):
                verify_trace(INITIAL + syscall, BINARY)

    def test_missing_wrong_failed_or_truncated_initial_exec_fails(self):
        for records in ['', INITIAL.replace('= 0', '= -1 ENOENT'), INITIAL.replace('\\x2f', '\\x30', 1), INITIAL.split(' = ')[0]]:
            with self.assertRaises(RuntimeError):
                verify_trace(records, BINARY)

    def test_network_syscalls_fail_without_any_browser_launch(self):
        for record in ['socket(AF_INET, SOCK_STREAM, IPPROTO_TCP) = 3',
                       'socket(AF_INET6, SOCK_DGRAM, 0) = 3',
                       'connect(3, 0x1234, 16) = -1 ENETUNREACH',
                       'sendto(3, "x", 1, 0, NULL, 0) = 1',
                       'sendmsg(3, 0x1234, 0) = 1', 'sendmmsg(3, 0x1234, 1, 0) = 1']:
            with self.subTest(record=record), self.assertRaises(RuntimeError):
                verify_trace(INITIAL + record, BINARY)


if __name__ == '__main__':
    unittest.main()
