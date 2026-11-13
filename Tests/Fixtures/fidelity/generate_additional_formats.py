"""Small authored CC0-1.0 legacy PowerPoint fixture; no third-party deck data."""
from pathlib import Path
from struct import pack, pack_into
ROOT = Path(__file__).parent

def record(t, body=b'', vi=0): return pack('<HHI',vi,t,len(body))+body
def container(t, body): return record(t,body,15)
def text(value, kind=4): return record(0xf9f,bytes([kind,0,0,0]))+record(0xfa0,value.encode('utf-16le'))
def cell(box, value): return container(0xf004,record(0xf00f,pack('<4i',*box))+container(0xf00d,text(value)))
flag=container(0xf004,record(0xf122,pack('<HI',0x039f,1),0x13))
grid=container(0xf003,flag+b''.join(cell((c*100,r*100,(c+1)*100,(r+1)*100),value) for r,row in enumerate([['Fruit','Quantity','Price'],['Apple','5','$0.50'],['Pear','3','$0.75']]) for c,value in enumerate(row)))
link=container(0xfd7,record(0xfd3,pack('<I',21))+record(0xfba,'https://example.org/fruit'.encode('utf-16le'),0x10))
click=bytearray(16);pack_into('<I',click,4,21);click[8]=4;click[12]=8
shape=container(0xf004,container(0xf00d,text('Visit 🍎 fruit')+container(0xff2,record(0xff3,click))+record(0xfdf,pack('<II',6,14))))
slides=[container(0x3ee,text('Fruit data',0)+grid),container(0x3ee,text('Field guide',0)+shape)]
slide_list=b''.join(record(0x3f3,pack('<4I',i+2,0,0,100+i)) for i in range(2))
doc=container(0x3e8,container(0x409,link)+container(0xff0,slide_list))
stream=doc;offsets=[0]
for slide in slides: offsets.append(len(stream));stream+=slide
persist=len(stream);stream+=record(0x1772,pack('<I',1|(3<<20))+pack('<3I',*offsets))
edit=len(stream);atom=bytearray(28);pack_into('<I',atom,12,persist);pack_into('<I',atom,16,1);stream+=record(0xff5,atom)
assert len(stream)<4096
current=bytearray(4096);pack_into('<I',current,16,edit)
FREE=0xffffffff;END=0xfffffffe
header=bytearray(512);header[:8]=bytes.fromhex('D0CF11E0A1B11AE1');pack_into('<6H',header,24,0x3e,3,0xfffe,9,6,0)
pack_into('<9I',header,40,0,1,1,0,4096,END,0,END,0)
pack_into('<109I',header,76,0,*([FREE]*108))
fat=[FREE]*128;fat[0]=0xfffffffd;fat[1]=END
for start in [2,10]:
 for i in range(start,start+8):fat[i]=i+1 if i<start+7 else END

def entry(name,kind,start=END,size=0,child=FREE,left=FREE,right=FREE):
 e=bytearray(128);raw=(name+'\0').encode('utf-16le');e[:len(raw)]=raw;pack_into('<H',e,64,len(raw));e[66]=kind;e[67]=1
 pack_into('<III',e,68,left,right,child);pack_into('<I',e,116,start);pack_into('<Q',e,120,size);return e
entries=entry('Root Entry',5,child=1)+entry('PowerPoint Document',2,2,4096,left=2)+entry('Current User',2,10,4096)+bytes(128)
(ROOT/'legacy-tables-links.ppt').write_bytes(header+pack('<128I',*fat)+entries+stream.ljust(4096,b'\0')+current)
