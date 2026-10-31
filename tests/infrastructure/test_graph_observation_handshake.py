"""Pure FIFO/selector models and source controls; no process, pipe or SQL opened."""
import ast
import copy
import hashlib
import json
import os
import re
from pathlib import Path
import stat
import types
import unittest
from unittest import mock

ROOT=Path(__file__).resolve().parents[2]
DRIVER=ROOT/'apps/desktop/tests/factory-child-goal-real-ipc.py'
IDENTITY={'pid':2001,'ppid':2000,'start_ticks':'4567','uid':1000,'pgid':2001}
NONCE='a'*32
PIN=[1,2,1000,0o700]


def sha(raw):return hashlib.sha256(raw).hexdigest()


def strip_handshake_layer(text):
    fixture=json.loads((ROOT/'tests/infrastructure/fixtures/graph_observation_handshake/source_inverse.json').read_text())
    start=text.index('GRAPH_OBSERVATION_ACK_CODE = ');end=text.index('def generated_acp():',start)
    if sha(text[start:end].encode())!=fixture['addition_sha256']:raise AssertionError('Exact handshake addition changed')
    text=text[:start]+text[end:]
    for before,after in reversed(fixture['replacements']):
        if text.count(after)!=1:raise AssertionError('Exact handshake inverse anchor changed')
        text=text.replace(after,before,1)
    return text


def shared():
    tree=ast.parse(DRIVER.read_text())
    node=next(n for n in tree.body if isinstance(n,ast.Assign)and any(isinstance(t,ast.Name)and t.id=='GRAPH_OBSERVATION_ACK_CODE'for t in n.targets))
    code=ast.literal_eval(node.value)
    ns={'os':os,'re':re,'Path':Path,'time':types.SimpleNamespace(monotonic_ns=lambda:1_000_000_000),'json':json}
    exec(compile(code,'<fixed-generated-ACK-helper>','exec'),ns)
    return ns,code


def function(ns,name):
    node=next(n for n in ast.parse(DRIVER.read_text()).body if isinstance(n,ast.FunctionDef)and n.name==name)
    exec(compile(ast.Module(body=[node],type_ignores=[]),'<fixed-host-function>','exec'),ns)
    return ns[name]


def require(value,*_):
    if not value:raise AssertionError('Fixed handshake refused')


def fifo_info(**changes):
    values={'st_dev':1,'st_ino':3,'st_uid':1000,'st_mode':stat.S_IFIFO|0o600,'st_nlink':1}
    values.update(changes);return types.SimpleNamespace(**values)


class CodecAndMetadata(unittest.TestCase):
    def test_ack_binds_nonce_pid_birth_and_complete_initial_metadata_digest(self):
        ns,_=shared();raw=ns['graph_ack_bytes'](NONCE,IDENTITY);value=json.loads(raw)
        self.assertEqual(set(value),{'contract','nonce','pid','start_ticks','metadata_sha256'})
        self.assertEqual(value['metadata_sha256'],sha(json.dumps(IDENTITY,sort_keys=True,separators=(',',':')).encode()))
        self.assertLessEqual(len(raw),4096)
        for key,bad in [('pid',2002),('start_ticks','4568'),('ppid',1999),('uid',1001)]:
            changed=dict(IDENTITY,**{key:bad})
            if key=='pid':changed['pgid']=bad
            self.assertNotEqual(ns['graph_ack_bytes'](NONCE,changed),raw)
        for nonce in ['',True,'A'*32,'a'*33,'a'*31]:
            with self.assertRaises(AssertionError):ns['graph_ack_bytes'](nonce,IDENTITY)
        for key,bad in [('pid',True),('ppid',0),('uid',False),('pgid',2002),('start_ticks',1)]:
            with self.assertRaises(AssertionError):ns['graph_ack_bytes'](NONCE,dict(IDENTITY,**{key:bad}))

    def test_fifo_type_mode_owner_links_and_noninheritance_are_strict(self):
        ns,_=shared();fake=types.SimpleNamespace(geteuid=lambda:1000,set_inheritable=mock.Mock(),get_inheritable=lambda fd:False,close=mock.Mock())
        ns['os']=fake;self.assertEqual(ns['graph_ack_noinherit'](20),20);fake.set_inheritable.assert_called_once_with(20,False)
        self.assertEqual(ns['graph_ack_fifo_pin'](fifo_info()),(1,3,stat.S_IFIFO|0o600,1000,1))
        for info in [fifo_info(st_mode=stat.S_IFLNK|0o600),fifo_info(st_mode=stat.S_IFREG|0o600),
            fifo_info(st_mode=stat.S_IFIFO|0o644),fifo_info(st_uid=1001),fifo_info(st_nlink=2)]:
            with self.assertRaises(AssertionError):ns['graph_ack_fifo_pin'](info)
        fake.get_inheritable=lambda fd:True
        with self.assertRaises(AssertionError):ns['graph_ack_noinherit'](20)

    def test_self_metadata_uses_bounded_kernel_shape_not_a_generated_record(self):
        ns,_=shared();tail=['S','2000']+['0']*17+['4567']
        raw=('2001 (synthetic) '+' '.join(tail)).encode();reads=[]
        class Stream:
            def __enter__(self):return self
            def __exit__(self,*_):return False
            def read(self,n):reads.append(n);return raw
        ns['open']=mock.Mock(return_value=Stream())
        ns['os']=types.SimpleNamespace(getpid=lambda:2001,geteuid=lambda:1000,getpgid=lambda pid:2001)
        self.assertEqual(ns['graph_ack_self_identity'](),IDENTITY)
        ns['open'].assert_called_once_with('/proc/self/stat','rb');self.assertEqual(reads,[4097])
        raw=b'x'*4097
        with self.assertRaises(AssertionError):ns['graph_ack_self_identity']()

    def test_directory_descriptor_walk_refuses_alias_replacement_wrong_mode_and_bool_pin(self):
        ns,_=shared();opened=[];closed=[]
        def open_path(name,flags,**kwargs):opened.append((name,flags,kwargs));return len(opened)+10
        info=types.SimpleNamespace(st_mode=stat.S_IFDIR|0o700,st_uid=1000,st_dev=1,st_ino=2)
        fake=types.SimpleNamespace(**{k:getattr(os,k)for k in ('O_RDONLY','O_DIRECTORY','O_NOFOLLOW','O_CLOEXEC')},
            open=open_path,close=closed.append,fstat=lambda fd:info,geteuid=lambda:1000,
            set_inheritable=lambda *a:None,get_inheritable=lambda fd:False)
        ns['os']=fake
        fd,pin=ns['graph_ack_directory']('/synthetic/owned',PIN)
        self.assertEqual(pin,PIN);self.assertEqual([n for n,_,_ in opened],['/','synthetic','owned'])
        self.assertTrue(all(flags&os.O_NOFOLLOW and flags&os.O_CLOEXEC for _,flags,_ in opened))
        for bad in [[1,3,1000,0o700],[True,2,1000,0o700]]:
            with self.assertRaises(AssertionError):ns['graph_ack_directory']('/synthetic/owned',bad)
        info.st_mode=stat.S_IFDIR|0o755
        with self.assertRaises(AssertionError):ns['graph_ack_directory']('/synthetic/owned')
        fake.open=mock.Mock(side_effect=OSError('Alias refused by NOFOLLOW'))
        with self.assertRaises(OSError):ns['graph_ack_directory']('/synthetic/alias')


class ProviderEventWait(unittest.TestCase):
    def environment(self,chunks,late=False):
        ns,code=shared();clock=types.SimpleNamespace(value=1_000_000_000);events=[];blocks=list(chunks)
        channel={'fd':20,'parent':10,'name':'fifo','pin':(1,3,stat.S_IFIFO|0o600,1000,1),
            'deadline_ns':5_000_000_000,'nonce':NONCE,'identity':copy.deepcopy(IDENTITY)}
        class Selector:
            def __enter__(self):return self
            def __exit__(self,*_):return False
            def register(self,fd,mask):events.append(('registered',fd,mask))
            def select(self,timeout):
                events.append(('select',timeout))
                if not blocks or late:clock.value+=int(timeout*1e9);return []if not blocks else[(types.SimpleNamespace(fd=20),1)]
                clock.value+=1_000_000;return[(types.SimpleNamespace(fd=20),1)]
        ns['selectors']=types.SimpleNamespace(DefaultSelector=Selector,EVENT_READ=1)
        ns['time']=types.SimpleNamespace(monotonic_ns=lambda:clock.value)
        ns['os']=types.SimpleNamespace(get_inheritable=lambda fd:False,geteuid=lambda:1000,
            read=lambda fd,n:(events.append(('read',n)),blocks.pop(0))[1],fstat=lambda fd:fifo_info(),stat=lambda *a,**k:fifo_info())
        ns['graph_ack_self_identity']=lambda:copy.deepcopy(IDENTITY)
        return ns,channel,clock,events,code

    def test_exact_and_fragmented_event_ack_stay_within_same_original_deadline(self):
        proto,_=shared();raw=proto['graph_ack_bytes'](NONCE,IDENTITY)
        for chunks in [[raw],[raw[:7],raw[7:]]]:
            ns,channel,clock,events,code=self.environment(chunks)
            ns['graph_ack_wait'](channel);self.assertLess(clock.value,channel['deadline_ns'])
            self.assertTrue(all(e[1]<=4 for e in events if e[0]=='select'))
            self.assertNotIn('time.sleep',code)

    def test_wrong_nonce_birth_metadata_extra_truncated_or_late_ack_fails(self):
        proto,_=shared();raw=proto['graph_ack_bytes'](NONCE,IDENTITY);value=json.loads(raw)
        bad=[]
        for key,replacement in [('nonce','b'*32),('pid',2002),('start_ticks','4568'),('metadata_sha256','0'*64),('extra','unknown')]:
            changed=dict(value,**{key:replacement});bad.append((json.dumps(changed)+'\n').encode())
        bad.extend([b'x'*4097,b'',raw[:-1]])
        for packet in bad:
            ns,channel,_,_,_=self.environment([packet])
            with self.subTest(size=len(packet)),self.assertRaises(AssertionError):ns['graph_ack_wait'](channel)
        for chunks,late in [([],False),([raw],True)]:
            ns,channel,_,_,_=self.environment(chunks,late)
            with self.assertRaises(AssertionError):ns['graph_ack_wait'](channel)
        ns,channel,_,_,_=self.environment([raw]);ns['graph_ack_self_identity']=lambda:dict(IDENTITY,ppid=1999)
        with self.assertRaises(AssertionError):ns['graph_ack_wait'](channel)
        ns,channel,clock,_,_=self.environment([raw])
        def late_identity():clock.value=channel['deadline_ns'];return copy.deepcopy(IDENTITY)
        ns['graph_ack_self_identity']=late_identity
        with self.assertRaises(AssertionError):ns['graph_ack_wait'](channel)

    def test_prepare_uses_original_started_epoch_and_reservation_with_closed_fds(self):
        ns,_=shared();opened=[];closed=[];mkfifo=mock.Mock()
        fake=types.SimpleNamespace(**{k:getattr(os,k)for k in ('O_RDWR','O_NONBLOCK','O_NOFOLLOW','O_CLOEXEC')},
            mkfifo=mkfifo,stat=lambda *a,**k:fifo_info(),open=lambda *a,**k:(opened.append((a,k)),20)[1],
            fstat=lambda fd:fifo_info(),geteuid=lambda:1000,close=closed.append,
            set_inheritable=mock.Mock(),get_inheritable=lambda fd:False)
        ns['os']=fake;ns['graph_ack_self_identity']=lambda:copy.deepcopy(IDENTITY)
        ns['graph_ack_directory']=mock.Mock(return_value=(10,PIN))
        for case,runtime in [('normal',4000),('lost-start',4000),('cancel',10000)]:
            settings={'case':case,'nonce':NONCE,'graph_observation_directory':'/synthetic','graph_observation_directory_pin':PIN}
            declared={'runtime_ms':runtime};request={'node':{'reservation':declared},'budget':{'reservation':declared}}
            channel=ns['graph_ack_prepare'](settings,request,500_000_000)
            self.assertEqual(channel['deadline_ns'],500_000_000+runtime*1_000_000)
            self.assertTrue(opened[-1][0][1]&os.O_NONBLOCK and opened[-1][0][1]&os.O_CLOEXEC)
            ns['graph_ack_close'](channel);self.assertEqual(closed[-2:],[20,10])
        mkfifo.side_effect=FileExistsError('Unique FIFO exists')
        with self.assertRaises(FileExistsError):ns['graph_ack_prepare'](settings,request,500_000_000)
        self.assertEqual(closed[-1],10)
        for runtime in (True,0,16000):
            request={'node':{'reservation':{'runtime_ms':runtime}},'budget':{'reservation':{'runtime_ms':runtime}}}
            with self.assertRaises(AssertionError):ns['graph_ack_prepare'](settings,request,500_000_000)


class HostAndReadonlyObservation(unittest.TestCase):
    def test_only_sealed_initial_binding_can_publish_one_atomic_noninherited_ack(self):
        ns,_=shared();writes=[];opened=[];closed=[]
        class Writer:
            def __enter__(self):return self
            def __exit__(self,*_):return False
            def write(self,raw):writes.append(('receipt',raw))
            def flush(self):pass
            def fileno(self):return 30
        fake=types.SimpleNamespace(**{k:getattr(os,k)for k in ('O_WRONLY','O_NONBLOCK','O_NOFOLLOW','O_CLOEXEC','O_CREAT','O_EXCL')},
            geteuid=lambda:1000,stat=lambda *a,**k:fifo_info(),fstat=lambda fd:fifo_info(),
            open=lambda *a,**k:(opened.append((a,k)),20 if len(opened)==1 else 30)[1],
            fpathconf=lambda *a:4096,fdopen=lambda *a:Writer(),fsync=lambda fd:None,
            write=lambda fd,raw:(writes.append(('pipe',raw)),len(raw))[1],close=closed.append,
            set_inheritable=mock.Mock(),get_inheritable=lambda fd:False)
        ns.update(os=fake,require=require,GRAPH_IDENTITY_FIELDS=('pid','ppid','start_ticks','uid','pgid'))
        function(ns,'graph_valid_identity');ns['graph_ack_directory']=mock.Mock(return_value=(10,PIN))
        publish=function(ns,'publish_graph_binding_ack');fixed={'path':'fixed','owned_provider':{k:IDENTITY[k]for k in ('pid','pgid','start_ticks')}}
        context={'nonce':NONCE,'_graph_provider_identities':{'fixed':copy.deepcopy(IDENTITY)},
            'graph_observation_directory':'/synthetic','graph_observation_directory_pin':PIN}
        for bad in [{},dict(context,_graph_provider_identities={}),dict(context,_graph_provider_identities={'fixed':dict(IDENTITY,uid=1001)})]:
            with self.assertRaises((AssertionError,KeyError)):publish(fixed,bad)
        self.assertEqual(writes,[])
        publish(fixed,context)
        self.assertEqual([x[0]for x in writes],['receipt','pipe']);self.assertEqual(writes[0][1],writes[1][1])
        self.assertTrue(opened[1][0][1]&os.O_EXCL and opened[1][0][1]&os.O_NOFOLLOW)
        with self.assertRaises(AssertionError):publish(fixed,context)
        self.assertEqual(len(writes),2)

    def test_ui_enabled_wait_keeps_existing_pump_capture_active_without_new_writes_or_deadline(self):
        ns={'time':types.SimpleNamespace(monotonic=lambda:clock[0]),'require':require};fn=function(ns,'wait_graph_recovery_enabled')
        clock=[0.0];captured=[];probes=[];assertions=[]
        class Pending(Exception):pass
        class Locator:
            def is_enabled(self,timeout):
                probes.append(timeout)
                if len(captured)==1:raise Pending()
                return len(captured)>=3
        def wait(read,predicate,seconds):
            self.assertEqual(seconds,15);end=clock[0]+seconds
            while True:
                captured.append(clock[0]);clock[0]+=.01
                value=read()
                if predicate(value):return value
                require(clock[0]<end);clock[0]+=.02
        self.assertTrue(fn(Locator(),assertions.append,Pending,wait=wait))
        self.assertEqual(len(captured),3);self.assertEqual(probes,[20]*3)
        self.assertEqual(len(assertions),1);self.assertLessEqual(assertions[0],15000);self.assertLess(clock[0],15)
        def late_wait(read,predicate,seconds):clock[0]+=15;return read()
        with self.assertRaises(AssertionError):fn(Locator(),assertions.append,Pending,wait=late_wait)

    def test_ui_late_enabled_reply_or_enabled_assertion_never_renews_window(self):
        for where in ('probe','assertion'):
            clock=[0.0];ns={'time':types.SimpleNamespace(monotonic=lambda:clock[0]),'require':require};fn=function(ns,'wait_graph_recovery_enabled')
            class Locator:
                def is_enabled(self,timeout):
                    if where=='probe':clock[0]+=15
                    return True
            def assertion(timeout):
                if where=='assertion':clock[0]+=15
            def wait(read,predicate,seconds):return read()
            with self.subTest(where=where),self.assertRaises(AssertionError):fn(Locator(),assertion,TimeoutError,wait=wait)


class SourceControls(unittest.TestCase):
    def test_exact_handshake_layer_inverse_preserves_all_old_harness_business_budget_and_cleanup(self):
        text=DRIVER.read_text();restored=strip_handshake_layer(text)
        self.assertEqual(sha(restored.encode()),'b7cac9f5eda5d8933de6b739da65d2246063fea3ce29272cf33963ca0dc0239a')
        self.assertIn("publish_graph_binding_ack(fixed,context)",text)
        self.assertIn("graph_observation_directory=root/'graph-observation'",text)
        self.assertIn('not graph_observation_directory.is_relative_to(wroot.resolve(strict=True))',text)
        self.assertIn('not graph_observation_directory.is_relative_to(source_root.resolve(strict=True))',text)
        tree=ast.parse(text);fn=next(n for n in tree.body if isinstance(n,ast.FunctionDef)and n.name=='generated_acp')
        prompt=ast.literal_eval(next(n.value for n in fn.body if isinstance(n,ast.Assign)and any(isinstance(t,ast.Name)and t.id=='graph_prompt'for t in n.targets)))
        self.assertLess(prompt.index('graph_ack_prepare'),prompt.index("os.replace(record+'.tmp',record)"))
        self.assertLess(prompt.index("os.replace(record+'.tmp',record)"),prompt.index('graph_ack_wait'))
        self.assertLess(prompt.index('graph_ack_close'),prompt.index("output=c['outputs']"))
        self.assertIn('graph_ack_prepare(settings,c,FIXTURE_STARTED_NS)',prompt)
        self.assertNotIn('publish_graph_binding_ack',prompt)
        bind=next(n for n in tree.body if isinstance(n,ast.FunctionDef)and n.name=='bind_graph_observation')
        branches=[n for n in ast.walk(bind)if isinstance(n,ast.If)and ast.unparse(n.test)=='old is None']
        calls=[n for n in ast.walk(branches[0])if isinstance(n,ast.Call)and isinstance(n.func,ast.Name)and n.func.id=='publish_graph_binding_ack']
        self.assertEqual(len(calls),1)

    def test_updated_lost_start_ast_guard_keeps_five_original_negative_classes_and_poll_slice(self):
        ns={};check=function(ns,'lost_start_observation_self_check');text=DRIVER.read_text();self.assertTrue(check(text))
        for old,new in [('deadline=time.monotonic()+15\n    def read_enabled','deadline=time.monotonic()+16\n    def read_enabled'),
            ('deadline=time.monotonic()+15\n    def read_enabled','deadline=time.monotonic()+0\n    def read_enabled'),
            ('timeout=min(20,remaining)','timeout=15000'),('seconds=15)\n\n\ndef generated_acp','seconds=16)\n\n\ndef generated_acp')]:
            self.assertIn(old,text)
            with self.subTest(old=old),self.assertRaises(AssertionError):check(text.replace(old,new,1))
        fixture=json.loads((ROOT/'tests/infrastructure/fixtures/graph_observation_handshake/source_inverse.json').read_text())
        runtime=fixture['replacements'][-1][1]
        call="wait_graph_recovery_enabled(recover,lambda timeout:expect(recover).to_be_enabled(timeout=timeout),PlaywrightTimeoutError,wait=wait)"
        for changed in [runtime.replace(call,'expect(recover).to_be_enabled()'),runtime.replace('exact=True','exact=False'),runtime.replace('            wait_graph_recovery_enabled', '            recover.click()\n            wait_graph_recovery_enabled')]:
            with self.assertRaises(AssertionError):check(text.replace(runtime,changed,1))


if __name__=='__main__':unittest.main()
