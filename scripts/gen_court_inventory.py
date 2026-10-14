import os, re, datetime
ROOT = "/Users/sac/strata"
dirs = ["strata_protocol/tests", "temprun/tests", "tests", "stratus/test"]
pats = ("*_court.rs","*_court*_test.rs","*_test.rs","*_test.exs","*_court.exs")
import glob
files=[]
for d in dirs:
    files += glob.glob(os.path.join(ROOT,d,"**",pats[0]), recursive=True)
    files += glob.glob(os.path.join(ROOT,d,"**",pats[1]), recursive=True)
    files += glob.glob(os.path.join(ROOT,d,"**",pats[2]), recursive=True)
    files += glob.glob(os.path.join(ROOT,d,"**",pats[3]), recursive=True)
    files += glob.glob(os.path.join(ROOT,d,"**",pats[4]), recursive=True)
files=sorted(set(files))
sections=[("strata_protocol (Rust)", "strata_protocol/tests/"),
          ("temprun (Rust)", "temprun/tests/"),
          ("strata root (Rust)", "tests/"),
          ("stratus (Elixir)", "stratus/test/")]
def doc_of(f):
    src=open(f,encoding='utf-8',errors='replace').read()
    if f.endswith('.rs'):
        m=re.findall(r'^//!\s?(.*)$',src,re.M)[:2]
        d=' '.join(x.strip() for x in m).strip()
        d=re.sub(r'//!\s?','',d).strip()
    else:
        m=re.search(r'@moduledoc\s*\n?\s*"([^"]{0,160})',src)
        d=m.group(1).strip() if m else ''
    return d or "(no header doc)"
def target_of(f):
    src=open(f,encoding='utf-8',errors='replace').read()
    if f.endswith('.rs'):
        m=re.search(r'^use (?:strata_protocol|temprun|strata)::([\w:]+)',src,re.M)
        return m.group(1) if m else "—"
    m=re.search(r'^\s*alias ([\w.]+)',src,re.M)
    return m.group(1) if m else "—"
def sig_of(f):
    src=open(f,encoding='utf-8',errors='replace').read()
    s=[]
    if 'VACUITY' in src: s.append("VACUITY marker")
    if f.endswith('.rs') and 'should_panic' in src: s.append("should_panic")
    if f.endswith('.exs') and re.search(r'assert_raise',src): s.append("assert_raise")
    if re.search(r'refus',src,re.I): s.append("refusal assertions")
    return "+".join(s) if s else "none"
lines=[]
lines.append("# Court Inventory\n")
lines.append(f"Generated `python3 scripts/gen_court_inventory.py` on {datetime.date.today().isoformat()}. Regenerate on court additions.\n")

lines.append("Row per court/test file. `vacuity-signal` is indicative only (grep for VACUITY marker / should_panic / assert_raise / refusal assertions), NOT proof a court is non-vacuous.\n")
counts={'with':0,'without':0}
for title, prefix in sections:
    sel=[f for f in files if (ROOT+'/'+prefix) in f]
    lines.append(f"## {title} — `{prefix}`\n")
    lines.append("| file | target | law pinned (from header/module doc) | vacuity-signal |")
    lines.append("|---|---|---|---|")
    for f in sel:
        rel=os.path.relpath(f,ROOT)
        sig=sig_of(f)
        if sig=="none": counts['without']+=1
        else: counts['with']+=1
        lines.append(f"| [{os.path.basename(f)}]({rel}) | `{target_of(f)}` | {doc_of(f).replace('|','/').strip()} | {sig} |")
    lines.append("")
lines.append(f"**Totals**: {len(files)} court/test files; {counts['with']} with a vacuity signal, {counts['without']} without.\n")
open("/Users/sac/strata/docs/COURT_INVENTORY.md","w").write('\n'.join(lines))
print(f"files={len(files)} with={counts['with']} without={counts['without']}")
