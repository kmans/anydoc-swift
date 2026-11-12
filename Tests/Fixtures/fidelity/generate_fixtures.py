"""Deterministic, synthetic regression inputs. Authored for Telescopo; CC0-1.0."""
from pathlib import Path
from zipfile import ZipFile, ZIP_DEFLATED, ZipInfo
import xml.etree.ElementTree as ET
ROOT = Path(__file__).parent
W = 'http://schemas.openxmlformats.org/wordprocessingml/2006/main'
C = 'http://schemas.openxmlformats.org/drawingml/2006/chart'

def archive(name, entries):
    with ZipFile(ROOT / name, 'w', ZIP_DEFLATED) as z:
        for path, content in sorted(entries.items()):
            info = ZipInfo(path, (2026, 11, 10, 0, 0, 0))
            info.compress_type = ZIP_DEFLATED
            z.writestr(info, content)

entries = {
 '[Content_Types].xml': '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>',
 '_rels/.rels': '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="r1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>',
 'word/_rels/document.xml.rels': '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="s" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>',
 'word/styles.xml': f'''<w:styles xmlns:w="{W}">
 <w:style w:type="character" w:styleId="Raised"><w:name w:val="Raised"/><w:rPr><w:vertAlign w:val="superscript"/></w:rPr></w:style>
 <w:style w:type="character" w:styleId="Child"><w:basedOn w:val="Raised"/></w:style>
 <w:style w:type="character" w:styleId="Reset"><w:basedOn w:val="Raised"/><w:rPr><w:vertAlign w:val="baseline"/></w:rPr></w:style>
 <w:style w:type="paragraph" w:styleId="CodeBase"><w:name w:val="Code"/><w:rPr><w:rFonts w:ascii="Menlo"/></w:rPr></w:style>
 <w:style w:type="paragraph" w:styleId="CodeChild"><w:basedOn w:val="CodeBase"/></w:style>
 </w:styles>''',
 'word/document.xml': f'''<w:document xmlns:w="{W}"><w:body>
 <w:p><w:r><w:t>a</w:t></w:r><w:r><w:rPr><w:rStyle w:val="Child"/></w:rPr><w:t>2</w:t></w:r><w:r><w:t xml:space="preserve"> b</w:t></w:r><w:r><w:rPr><w:rStyle w:val="Reset"/></w:rPr><w:t>3</w:t></w:r><w:r><w:t xml:space="preserve"> c</w:t></w:r><w:r><w:rPr><w:rStyle w:val="Child"/><w:vertAlign w:val="baseline"/></w:rPr><w:t>4</w:t></w:r></w:p>
 <w:p><w:pPr><w:pStyle w:val="CodeChild"/></w:pPr><w:r><w:t xml:space="preserve">    let value = 1</w:t></w:r></w:p>
 </w:body></w:document>'''
}
archive('inherited-styles.docx', entries)
cycle = dict(entries)
cycle['word/styles.xml'] = f'<w:styles xmlns:w="{W}"><w:style w:type="character" w:styleId="Child"><w:basedOn w:val="Child"/></w:style></w:styles>'
archive('cyclic-styles.docx', cycle)

with ZipFile(ROOT/'workbook.xlsx') as z: original = {n:z.read(n) for n in z.namelist()}
variant = dict(original)
# Force table fallback; use an unresolved reference with a valid chart cache;
# leave an explicit hole in that cache and prohibit zero filling.
r = ET.fromstring(variant['xl/charts/chart1.xml'])
r.find('.//{%s}grouping'%C).set('val','stacked')
variant['xl/charts/chart1.xml'] = ET.tostring(r)
r = ET.fromstring(variant['xl/charts/chart2.xml'])
r.find('.//{%s}val/{%s}numRef/{%s}f'%(C,C,C)).text = "'Missing sheet'!$A$1:$A$6"
cache = r.find('.//{%s}val/{%s}numRef/{%s}numCache'%(C,C,C))
point = next(p for p in cache.findall('{%s}pt'%C) if p.get('idx')=='2')
cache.remove(point)
variant['xl/charts/chart2.xml'] = ET.tostring(r)
archive('chart-fallbacks.xlsx', variant)
malformed = dict(original)
malformed['xl/charts/chart2.xml'] = b'<broken'
archive('malformed-chart.xlsx', malformed)
# This oversized claimed count must fail before any vector allocation.
bomb = dict(original)
r = ET.fromstring(bomb['xl/charts/chart2.xml'])
r.find('.//{%s}val/{%s}numRef/{%s}f'%(C,C,C)).text = 'Unknown!A1:A6'
r.find('.//{%s}val/{%s}numRef/{%s}numCache/{%s}ptCount'%(C,C,C,C)).set('val','4294967295')
bomb['xl/charts/chart2.xml'] = ET.tostring(r)
archive('oversized-chart-cache.xlsx', bomb)
# Unsupported bubble charts still retain X, Y, and size dimensions in tables.
bubble = dict(original)
r = ET.fromstring(bubble['xl/charts/chart1.xml'])
plot = r.find('.//{%s}barChart'%C)
plot.tag = '{%s}bubbleChart'%C
for series in plot.findall('{%s}ser'%C):
    cat = series.find('{%s}cat'%C); cat.tag = '{%s}xVal'%C
    ref = next(iter(cat)); ref.tag = '{%s}numRef'%C
    ref.find('{%s}f'%C).text = 'Unknown!A1:A4'
    cache = ref.find('{%s}strCache'%C); cache.tag = '{%s}numCache'%C
    for index, point in enumerate(cache.findall('{%s}pt'%C)):
        point.find('{%s}v'%C).text = str(index + 1)
    series.find('{%s}val'%C).tag = '{%s}yVal'%C
    sizes = ET.SubElement(series, '{%s}bubbleSize'%C)
    literal = ET.SubElement(sizes, '{%s}numLit'%C)
    ET.SubElement(literal, '{%s}ptCount'%C, {'val':'4'})
    for index in range(4):
        point = ET.SubElement(literal, '{%s}pt'%C, {'idx':str(index)})
        ET.SubElement(point, '{%s}v'%C).text = str((index+1)*10)
bubble['xl/charts/chart1.xml'] = ET.tostring(r)
archive('bubble-table.xlsx', bubble)
# A bar chart with a trend line is not a simple bar chart. Keep its source data
# and explicitly report that the extra visual detail cannot be represented.
trend = dict(original)
r = ET.fromstring(trend['xl/charts/chart1.xml'])
series = r.find('.//{%s}ser'%C)
line = ET.SubElement(series, '{%s}trendline'%C)
ET.SubElement(line, '{%s}trendlineType'%C, {'val':'linear'})
trend['xl/charts/chart1.xml'] = ET.tostring(r)
archive('trendline-table.xlsx', trend)
# Literal examples must never be mistaken for generated image destinations.
with ZipFile(ROOT/'handbook.docx') as z: literal = {n:z.read(n) for n in z.namelist()}
r = ET.fromstring(literal['word/document.xml'])
body = r.find('{%s}body'%W)
p = ET.SubElement(body, '{%s}p'%W)
pr = ET.SubElement(p, '{%s}pPr'%W)
ET.SubElement(pr, '{%s}pStyle'%W, {'{%s}val'%W:'Code'})
run = ET.SubElement(p, '{%s}r'%W)
ET.SubElement(run, '{%s}t'%W).text = '![literal](anydoc-asset:0) ![literal](telescopo-import-image:image-0.png)'
literal['word/document.xml'] = ET.tostring(r)
archive('literal-image-references.docx', literal)

# Small independent table-layout cases: titles, ambiguous rows, explicit headers,
# covered merged cells, and cached/uncached formulas. No spreadsheet engine runs.
S = 'http://schemas.openxmlformats.org/spreadsheetml/2006/main'
R = 'http://schemas.openxmlformats.org/officeDocument/2006/relationships'
def text_cell(ref, text):
    return f'<c r="{ref}" t="inlineStr"><is><t>{text}</t></is></c>'
def row(number, cells):
    return f'<row r="{number}">{cells}</row>'
layouts = {
 '[Content_Types].xml': f'<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/></Types>',
 '_rels/.rels': f'<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="r1" Type="{R}/officeDocument" Target="xl/workbook.xml"/></Relationships>',
 'xl/workbook.xml': f'<workbook xmlns="{S}" xmlns:r="{R}"><sheets>'+''.join(f'<sheet name="{name}" sheetId="{i}" r:id="r{i}"/>' for i,name in enumerate(['Titles','Ambiguous','Explicit','Formula caches'],1))+'</sheets></workbook>',
 'xl/_rels/workbook.xml.rels': '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'+''.join(f'<Relationship Id="r{i}" Type="{R}/worksheet" Target="worksheets/sheet{i}.xml"/>' for i in range(1,5))+'</Relationships>'
}
contents = [
 row(1,text_cell('A1','Quarterly results'))+row(2,text_cell('A2','Figures supplied by the source'))+row(3,text_cell('A3','Category')+text_cell('B3','Q1')+text_cell('C3','Q2'))+row(4,text_cell('A4','Revenue')+'<c r="B4"><v>10</v></c><c r="C4"><v>20</v></c>')+row(5,text_cell('A5','Merged value')+text_cell('C5','Tail')),
 row(1,text_cell('A1','Alpha')+'<c r="B1"><v>11</v></c>')+row(2,text_cell('A2','Beta')+'<c r="B2"><v>22</v></c>'),
 row(1,text_cell('A1','North')+text_cell('B1','South'))+row(2,text_cell('A2','Amber')+text_cell('B2','Blue'))+row(3,text_cell('A3','Green')+text_cell('B3','Violet')),
 row(1,text_cell('A1','Cached')+text_cell('B1','Uncached'))+row(2,'<c r="A2"><f>2+2</f><v>4</v></c><c r="B2"><f>40+2</f></c>')
]
for i,content in enumerate(contents,1):
 extra = '<mergeCells count="2"><mergeCell ref="A1:C1"/><mergeCell ref="A5:B5"/></mergeCells>' if i==1 else ''
 if i==3: extra += '<tableParts count="1"><tablePart r:id="table1"/></tableParts>'
 layouts[f'xl/worksheets/sheet{i}.xml'] = f'<worksheet xmlns="{S}" xmlns:r="{R}"><sheetData>{content}</sheetData>{extra}</worksheet>'
layouts['xl/worksheets/_rels/sheet3.xml.rels'] = f'<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="table1" Type="{R}/table" Target="../tables/table1.xml"/></Relationships>'
layouts['xl/tables/table1.xml'] = f'<table xmlns="{S}" id="1" name="Regions" displayName="Regions" ref="A1:B3" headerRowCount="1"><tableColumns count="2"><tableColumn id="1" name="North"/><tableColumn id="2" name="South"/></tableColumns></table>'
archive('table-layouts.xlsx', layouts)
# A repeated drawing must reuse the original asset bytes at both anchors.
import copy
with ZipFile(ROOT/'handbook.docx') as z: repeated = {n:z.read(n) for n in z.namelist()}
r = ET.fromstring(repeated['word/document.xml'])
body = r.find('{%s}body'%W)
paragraph = next(p for p in body.findall('{%s}p'%W) if p.find('.//{%s}drawing'%W) is not None)
body.insert(0, copy.deepcopy(paragraph))
repeated['word/document.xml'] = ET.tostring(r)
archive('repeated-images.docx', repeated)

# A readable sheet's missing numeric point wins over a stale chart cache.
missing = dict(original)
r = ET.fromstring(missing['xl/worksheets/sheet1.xml'])
cell = next(c for c in r.iter('{%s}c'%S) if c.get('r')=='B9')
for value in list(cell): cell.remove(value)
missing['xl/worksheets/sheet1.xml'] = ET.tostring(r)
archive('worksheet-chart-hole.xlsx', missing)

# Number-format inheritance, safe/unsupported links, valid empty formula caches,
# true Excel chartsheets, missing sheet parts and dynamic-range chart caches.
P = 'http://schemas.openxmlformats.org/package/2006/relationships'
follow = {k:v for k,v in layouts.items() if k in ['[Content_Types].xml','_rels/.rels']}
follow['xl/workbook.xml'] = f'<workbook xmlns="{S}" xmlns:r="{R}"><sheets><sheet name="Values" sheetId="1" r:id="v"/><sheet name="Date chart" sheetId="2" r:id="d"/><sheet name="Percent chart" sheetId="3" r:id="p"/><sheet name="Missing sheet" sheetId="4" r:id="m"/></sheets></workbook>'
follow['xl/_rels/workbook.xml.rels'] = f'<Relationships xmlns="{P}"><Relationship Id="v" Type="{R}/worksheet" Target="worksheets/values.xml"/><Relationship Id="d" Type="{R}/chartsheet" Target="chartsheets/dates.xml"/><Relationship Id="p" Type="{R}/chartsheet" Target="chartsheets/percent.xml"/><Relationship Id="m" Type="{R}/worksheet" Target="worksheets/missing.xml"/><Relationship Id="style" Type="{R}/styles" Target="styles.xml"/></Relationships>'
follow['xl/styles.xml'] = f'<styleSheet xmlns="{S}"><numFmts><numFmt numFmtId="164" formatCode="&quot;$&quot;#,##0.00"/></numFmts><cellXfs><xf numFmtId="0"/><xf numFmtId="164"/><xf numFmtId="9"/><xf numFmtId="14"/></cellXfs></styleSheet>'
follow['xl/worksheets/values.xml'] = f'''<worksheet xmlns="{S}" xmlns:r="{R}"><cols><col min="1" max="4" style="1"/><col min="4" max="4" style="3"/></cols><sheetData>
<row r="1"><c r="A1"><v>1618.5</v></c><c r="B1" s="0"><v>1618.5</v></c><c r="C1" s="2"><v>0.25</v></c><c r="D1"><v>43466</v></c></row>
<row r="2" s="2" customFormat="1"><c r="A2"><v>0.25</v></c><c r="B2" s="1"><v>-2</v></c><c r="C2" s="0"><v>0</v></c></row>
{row(3,text_cell('A3','Website')+text_cell('B3','Internal')+text_cell('C3','Unsafe')+text_cell('D3','Broken'))}
<row r="4"><c r="A4"><f>1+1</f></c><c r="B4" t="str"><f>IF(1,&quot;&quot;,&quot;x&quot;)</f><v/></c><c r="C4" s="0"><f>1-1</f><v>0</v></c><c r="D4" t="e"><f>1/0</f><v>#DIV/0!</v></c></row>
</sheetData><hyperlinks><hyperlink ref="A3" r:id="safe" location="details"/><hyperlink ref="B3" location="Values!A1"/><hyperlink ref="C3" r:id="unsafe"/><hyperlink ref="D3" r:id="broken"/></hyperlinks></worksheet>'''
follow['xl/worksheets/_rels/values.xml.rels'] = f'<Relationships xmlns="{P}"><Relationship Id="safe" Type="{R}/hyperlink" Target="https://example.org/guide_(new)?a=1&amp;b=2" TargetMode="External"/><Relationship Id="unsafe" Type="{R}/hyperlink" Target="javascript:alert(1)" TargetMode="External"/></Relationships>'
def numeric_cache(tag, values, code):
    return f'<c:{tag}><c:numRef><c:f>Unresolved!DynamicName</c:f><c:numCache><c:formatCode>{code}</c:formatCode><c:ptCount val="{len(values)}"/>'+''.join(f'<c:pt idx="{i}"><c:v>{v}</c:v></c:pt>' for i,v in enumerate(values))+f'</c:numCache></c:numRef></c:{tag}>'
for stem,values,code in [('dates',[71,53,58],'General'),('percent',[0.25,0.5,0.75],'0%')]:
    follow[f'xl/chartsheets/{stem}.xml'] = f'<chartsheet xmlns="{S}" xmlns:r="{R}"><drawing r:id="drawing"/></chartsheet>'
    follow[f'xl/chartsheets/_rels/{stem}.xml.rels'] = f'<Relationships xmlns="{P}"><Relationship Id="drawing" Type="{R}/drawing" Target="../drawings/{stem}.xml"/></Relationships>'
    follow[f'xl/drawings/{stem}.xml'] = f'<drawing xmlns:c="{C}" xmlns:r="{R}"><c:chart r:id="chart"/></drawing>'
    follow[f'xl/drawings/_rels/{stem}.xml.rels'] = f'<Relationships xmlns="{P}"><Relationship Id="chart" Type="{R}/chart" Target="../charts/{stem}.xml"/></Relationships>'
    follow[f'xl/charts/{stem}.xml'] = f'<c:chartSpace xmlns:c="{C}"><c:chart><c:plotArea><c:lineChart><c:grouping val="standard"/><c:ser><c:tx><c:v>Amount</c:v></c:tx>'+numeric_cache('cat',[43466,43467,43468],'m/d/yyyy')+numeric_cache('val',values,code)+f'</c:ser></c:lineChart><c:dateAx><c:numFmt formatCode="m/d/yyyy"/></c:dateAx><c:valAx><c:numFmt formatCode="{code}"/></c:valAx></c:plotArea></c:chart></c:chartSpace>'
archive('spreadsheet-followups.xlsx',follow)
epoch=dict(follow)
epoch['xl/workbook.xml']=epoch['xl/workbook.xml'].replace('<sheets>','<workbookPr date1904="1"/><sheets>')
epoch['xl/charts/dates.xml']=epoch['xl/charts/dates.xml'].replace('43466','0').replace('43467','1').replace('43468','2')
archive('spreadsheet-followups-1904.xlsx',epoch)
