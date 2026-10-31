import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const script = fileURLToPath(new URL('./test-windows-terminal.py', import.meta.url));
const python = process.env.PYTHON ?? (process.platform === 'win32' ? 'python' : 'python3');

test('Windows console validation refuses implicit execution and safely filters terminal controls', () => {
  const environment = { ...process.env };
  delete environment.TERMKEY_TEST_WINDOWS_CONSOLE;
  const refused = spawnSync(python, ['-B', script, '--binary', script], { env: environment, encoding: 'utf8' });
  assert.notEqual(refused.status, 0);
  assert.match(refused.stderr, /explicit disposable-runner console opt-in/);
  const checked = spawnSync(python, ['-B', '-c', String.raw`
import importlib.util,sys
spec=importlib.util.spec_from_file_location('console_validation',sys.argv[1]); module=importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
assert module.plain_text(b'\x1b[31mhello\x1b[0m')=='hello'
assert module.plain_text(b'\x1b]0;title\x07hello')=='hello'
assert module.plain_text(b'\x1b]0;title\x1b\\hello')=='hello'
`, script], { encoding: 'utf8' });
  assert.equal(checked.status, 0, checked.stderr);
});
