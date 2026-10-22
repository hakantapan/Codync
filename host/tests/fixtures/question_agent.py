# ACP agent that asks the user through `elicitation/create`, the way claude-agent-acp asks
# AskUserQuestion, then replies with JSON: whether the host offered forms, what it answered,
# and how many prompts it got. `number` asks for a field a question card can't show; `choice` asks a question with no text box;
# `refusal` sends claude-agent-acp's refusal-fallback dialog (no `toolCallId`, one `choice` field).
import json,sys,threading
def send(m): sys.stdout.write(json.dumps(m)+"\n"); sys.stdout.flush()
mode=sys.argv[1] if len(sys.argv)>1 else "ask"
pending={}; nid=[100]; sid="s1"; caps={}; prompts=[0]
QUESTION={"type":"object","properties":{
  "question_0":{"type":"string","title":"Database","oneOf":[
    {"const":"PostgreSQL","title":"PostgreSQL","description":"Relational"},{"const":"SQLite","title":"SQLite"}]},
  "question_0_custom":{"type":"string","title":"Other","description":"Type your own answer"}}}
NUMBER={"type":"object","properties":{"count":{"type":"integer","title":"How many?"}}}
CHOICE={"type":"object","properties":{"question_0":QUESTION["properties"]["question_0"]}}
REFUSAL={"type":"object","properties":{"choice":{"type":"string","oneOf":[
  {"const":"retry_fallback","title":"Retry with Opus","description":"The session continues on Opus."},
  {"const":"cancelled","title":"Keep the refusal","description":"You can send a new message."}]}}}
def upd(u): send({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":sid,"update":u}})
def turn(rid):
    prompts[0]+=1
    nid[0]+=1; pid=nid[0]
    ev=threading.Event(); pending[pid]=ev
    if mode=="refusal":
        params={"mode":"form","sessionId":sid,"message":"Fable declined this request. Retry with Opus?","requestedSchema":REFUSAL}
    else:
        schema={"number":NUMBER,"choice":CHOICE}.get(mode,QUESTION)
        params={"mode":"form","sessionId":sid,"toolCallId":"t1","message":"Which database?","requestedSchema":schema}
    send({"jsonrpc":"2.0","id":pid,"method":"elicitation/create","params":params})
    ev.wait()
    if ev.answer.get("action")=="cancel":
        send({"jsonrpc":"2.0","id":rid,"result":{"stopReason":"cancelled"}}); return
    reply={"form":"form" in caps.get("elicitation",{}),"answer":ev.answer,"prompts":prompts[0]}
    upd({"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":json.dumps(reply)}})
    send({"jsonrpc":"2.0","id":rid,"result":{"stopReason":"end_turn"}})
for line in sys.stdin:
    m=json.loads(line)
    if "method" not in m:
        ev=pending.pop(m["id"],None)
        if ev: ev.answer=m.get("result") or {"error":m.get("error")}; ev.set()
        continue
    meth=m["method"]
    if meth=="initialize":
        caps.update(m["params"].get("clientCapabilities",{}))
        send({"jsonrpc":"2.0","id":m["id"],"result":{"protocolVersion":1,"agentCapabilities":{"loadSession":False}}})
    elif meth=="session/new": send({"jsonrpc":"2.0","id":m["id"],"result":{"sessionId":sid}})
    elif meth=="session/prompt": threading.Thread(target=turn,args=(m["id"],)).start()
    elif meth=="session/cancel":
        for k,ev in list(pending.items()): ev.answer={"action":"cancel"}; ev.set()
