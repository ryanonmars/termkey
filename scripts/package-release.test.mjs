import assert from 'node:assert/strict';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import test from 'node:test';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const script = path.join(root, 'scripts/package-release.py');
const python = process.env.PYTHON ?? (process.platform === 'win32' ? 'python' : 'python3');

function runPython(source, args = []) {
  return spawnSync(python, ['-B', '-c', source, script, ...args], { encoding: 'utf8', cwd: root });
}

const harness = String.raw`
import importlib.util, pathlib, struct, sys, tempfile, zipfile, stat, json
spec=importlib.util.spec_from_file_location('packaging',sys.argv[1]); module=importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
def macho(cpu):
 return struct.pack('<8I',0xfeedfacf,cpu,0,2,1,24,0,0)+struct.pack('<6I',0x32,24,1,0xb0000,0xf0000,0)
def elf(machine,segment=1):
 ident=b'\x7fELF\x02\x01\x01'+bytes(9)
 header=ident+struct.pack('<HHIQQQIHHHHHH',2,machine,1,0,64,0,0,64,56,1,64,0,0)
 return header+struct.pack('<IIQQQQQQ',segment,5,0,0,0,120,120,4096)
def elf_needed(machine):
 data=bytearray(elf(machine)); struct.pack_into('<H',data,56,2)
 struct.pack_into('<Q',data,64+32,192); struct.pack_into('<Q',data,64+40,192)
 return bytes(data)+struct.pack('<IIQQQQQQ',2,4,176,0,0,16,16,8)+struct.pack('<QQ',1,0)
def pe(library=None):
 data=bytearray(1024); data[:2]=b'MZ'; struct.pack_into('<I',data,0x3c,0x80); data[0x80:0x84]=b'PE\0\0'
 struct.pack_into('<HHIIIHH',data,0x84,0x8664,1,0,0,0,240,0x22)
 optional=0x98; struct.pack_into('<H',data,optional,0x20b); struct.pack_into('<Q',data,optional+24,0x140000000); struct.pack_into('<I',data,optional+60,512); struct.pack_into('<I',data,optional+108,16)
 struct.pack_into('<8sIIII',data,optional+240,b'.text\0\0\0',512,0x1000,512,512)
 if library:
  struct.pack_into('<II',data,optional+120,0x1000,40); struct.pack_into('<5I',data,512,0,0,0,0x1040,0); data[576:576+len(library)+1]=library.encode()+b'\0'
 return bytes(data)
def binary(info):
 return macho(0x100000c if info['arch']=='arm64' else 0x1000007) if info['os']=='macos' else elf(183 if info['arch']=='arm64' else 62) if info['os']=='linux' else pe()
def reject(call):
 try: call()
 except ValueError: return
 raise AssertionError('invalid input was accepted')
`;

test('packages all five targets reproducibly with exact native payload and modes', async (t) => {
  const directory = await mkdtemp(path.join(tmpdir(), 'termkey-package-'));
  t.after(() => rm(directory, { recursive: true, force: true }));
  const result = runPython(harness + String.raw`
root=pathlib.Path(sys.argv[2])
for info in module.TARGETS:
 binary_path=root/info['target']; binary_path.write_bytes(binary(info))
 first=module.package(info,binary_path,root/'one','1.2.3'); second=module.package(info,binary_path,root/'two','1.2.3')
 assert first.read_bytes()==second.read_bytes()
 module.verify(info,first)
 with zipfile.ZipFile(first) as archive:
  assert sorted(archive.namelist())==sorted(['LICENSE','README.txt','THIRD-PARTY-LICENSES.txt',info['executable']])
  assert stat.S_IMODE(archive.getinfo(info['executable']).external_attr>>16)==0o755
  assert all(item.date_time==module.STAMP for item in archive.infolist())
  assert info['target'] in archive.read('README.txt').decode()
print('all five target packages passed')
`, [directory]);
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /all five target packages passed/);
});

test('rejects wrong architectures, dynamic Linux binaries, runtime DLLs and malformed executables', () => {
  const result = runPython(harness + String.raw`
mac=module.target_info('aarch64-apple-darwin'); linux=module.target_info('x86_64-unknown-linux-musl'); windows=module.target_info('x86_64-pc-windows-msvc')
reject(lambda: module.validate_binary(macho(0x1000007),mac))
reject(lambda: module.validate_binary(elf(183),linux))
reject(lambda: module.validate_binary(elf(62,3),linux))
reject(lambda: module.validate_binary(elf_needed(62),linux))
reject(lambda: module.validate_binary(pe('VCRUNTIME140.dll'),windows))
reject(lambda: module.validate_binary(pe('untrusted.dll'),windows))
module.validate_binary(pe('KERNEL32.dll'),windows)
for info in module.TARGETS:
 for data in [b'',b'not an executable',binary(info)[:20]]:
  reject(lambda: module.validate_binary(data,info))
reject(lambda: module.version_value('../1.2.3'))
`);
  assert.equal(result.status, 0, result.stderr);
});

test('rejects ZIP traversal, extra files, symlinks, bad modes, duplicate names and missing supporting files', () => {
  const result = runPython(harness + String.raw`
info=module.target_info('x86_64-unknown-linux-musl')
with tempfile.TemporaryDirectory() as temporary:
 root=pathlib.Path(temporary); source=root/'binary'; source.write_bytes(binary(info)); original=module.package(info,source,root,'1.2.3')
 with zipfile.ZipFile(original) as archive: members={name:archive.read(name) for name in archive.namelist()}
 for bad in ['traversal','extra','symlink','mode','duplicate','missing','target','notices']:
  altered=root/(bad+'.zip')
  with zipfile.ZipFile(altered,'w') as archive:
   for name,data in members.items():
    if bad=='missing' and name=='LICENSE': continue
    if bad=='target' and name=='README.txt': data=b'wrong target'
    if bad=='notices' and name=='THIRD-PARTY-LICENSES.txt': data=b'Cargo.lock SHA-256: '+b'0'*64
    entry=zipfile.ZipInfo(name,module.STAMP); entry.create_system=3
    mode=0o755 if name=='termkey' else 0o644
    entry.external_attr=((stat.S_IFLNK if bad=='symlink' and name=='termkey' else stat.S_IFREG)|mode)<<16
    if bad=='mode' and name=='termkey': entry.external_attr=(stat.S_IFREG|0o644)<<16
    archive.writestr(entry,data)
   if bad in ['traversal','extra','duplicate']: archive.writestr('../secret' if bad=='traversal' else 'extra' if bad=='extra' else 'termkey',b'payload')
  reject(lambda: module.verify(info,altered))
`);
  assert.equal(result.status, 0, result.stderr);
});

test('manifest has exactly the requested targets and preserves existing Apple Silicon ZIP name', async () => {
  const targets = JSON.parse(await readFile(path.join(root, 'scripts/release-targets.json'), 'utf8'));
  assert.equal(targets.length, 5);
  assert.equal(new Set(targets.map(item => item.target)).size, 5);
  assert.equal(new Set(targets.map(item => item.archive)).size, 5);
  assert.equal(targets.find(item => item.target === 'aarch64-apple-darwin').archive, 'termkey-macos-aarch64.zip');
  assert.equal(targets.find(item => item.os === 'windows').executable, 'termkey.exe');
});

test('native smoke rejects a stale executable even when its archive has the requested version', () => {
  const result = runPython(harness + String.raw`
module.platform.system=lambda:'Linux'
module.platform.machine=lambda:'x86_64'
info=module.target_info('x86_64-unknown-linux-musl')
version='1.2.3'
def smoke(arguments,**kwargs):
 output='termkey '+version+'\n' if arguments[-1]=='--version' else 'Encrypted storage for synthetic fixtures'
 return module.subprocess.CompletedProcess(arguments,0,stdout=output,stderr='')
module.subprocess.run=smoke
module.verify_native(info,pathlib.Path('/synthetic/executable'),'1.2.3')
version='1.0.0'
reject(lambda:module.verify_native(info,pathlib.Path('/synthetic/executable'),'1.2.3'))
`);
  assert.equal(result.status, 0, result.stderr);
});

test('trusted native Mac ZIP verification requires an available notarization ticket on both architectures', () => {
  const result = runPython(harness + String.raw`
module.platform.system=lambda:'Darwin'
team='ABCDEFGHIJ'
calls=[]
deny_ticket=False
def signed_smoke(arguments,**kwargs):
 calls.append(arguments)
 if arguments[0]=='codesign':
  if '--verify' in arguments:
   assert '--check-notarization' in arguments
   assert '-R=notarized' in arguments
   assert kwargs['check'] is True
   if deny_ticket:
    raise module.subprocess.CalledProcessError(1,arguments,stderr='notarization ticket unavailable')
   return module.subprocess.CompletedProcess(arguments,0,stdout='',stderr='')
  assert arguments[1:3]==['-dv','--verbose=4']
  return module.subprocess.CompletedProcess(arguments,0,stdout='',stderr='Authority=Developer ID Application: Synthetic Tests ('+team+')\nTeamIdentifier='+team+'\nTimestamp=synthetic timestamp\nRuntime Version=15.0.0\n')
 output='termkey 1.2.3\n' if arguments[-1]=='--version' else 'Encrypted storage for synthetic fixtures'
 return module.subprocess.CompletedProcess(arguments,0,stdout=output,stderr='')
module.subprocess.run=signed_smoke
with tempfile.TemporaryDirectory() as temporary:
 root=pathlib.Path(temporary)
 for info in module.TARGETS:
  if info['os']!='macos': continue
  module.platform.machine=lambda:'arm64' if info['arch']=='arm64' else 'x86_64'
  source=root/info['target']; source.write_bytes(binary(info))
  archive=module.package(info,source,root,'1.2.3')
  calls.clear(); deny_ticket=False
  module.verify(info,archive,run=True,require_signature=True,team_id=team)
  assert len(calls)==4
  assert calls[0][:6]==['codesign','--verify','--strict','-R=notarized','--check-notarization','--verbose=2']
  assert [call[-1] for call in calls[2:]]==['--version','--help']
  calls.clear(); deny_ticket=True
  try:
   module.verify(info,archive,run=True,require_signature=True,team_id=team)
  except module.subprocess.CalledProcessError: pass
  else: raise AssertionError('trusted ZIP with no notarization ticket was accepted')
  assert len(calls)==1, 'a tool without a verified ticket must not be executed'
  calls.clear()
  module.verify(info,archive,run=True)
  assert [call[-1] for call in calls]==['--version','--help'], 'unsigned validation must not require Apple services'
`);
  assert.equal(result.status, 0, result.stderr);
});
