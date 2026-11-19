import QtQuick
import Quickshell.Io

// Bridge to `omamail serve` (~/.local/lib/omamail): one JSON object per line each way. The
// process lives exactly as long as the window — the panel Loader unloads
// this item on close, which ends the process and its IMAP connections.
Item {
  id: root

  property string binary: ""
  property bool started: false
  property int nextId: 1
  property var callbacks: ({})
  property string session: ""
  signal ready()

  signal changed(string folder)
  signal failed(string message)

  function request(cmd, args, callback) {
    if (!started || (cmd !== "status" && !session)) {
      if (callback) callback(false, "The mail backend is not ready", "error")
      return -1
    }
    var msg = {}
    for (var k in (args || {})) msg[k] = args[k]
    msg.id = nextId++
    msg.cmd = cmd
    msg.session = session
    if (callback) callbacks[msg.id] = { session: session, call: callback }
    var line = JSON.stringify(msg) + "\n"
    proc.write(line)
    return msg.id
  }

  function handleLine(line) {
    var msg
    try {
      msg = JSON.parse(line)
    } catch (e) {
      console.warn("omamail: unreadable backend line")
      return
    }
    if (msg.event === "changed") {
      if (root.session && msg.session === root.session) root.changed(msg.folder || "INBOX")
      return
    }
    var cb = callbacks[msg.id]
    if (!cb) return
    delete callbacks[msg.id]
    if (cb.session !== root.session) return
    cb.call(msg.ok === true, msg.ok ? msg.data : (msg.error || "Unknown error"), msg.code || "")
  }

  Process {
    id: proc
    command: [root.binary, "serve"]
    running: root.binary !== ""
    stdinEnabled: true

    stdout: SplitParser {
      onRead: function(line) { root.handleLine(line) }
    }
    stderr: SplitParser {
      onRead: function(line) { console.warn("omamail:", line) }
    }

    onStarted: {
      root.started = true
      root.ready()
    }

    onExited: function(code, status) {
      root.started = false
      root.callbacks = ({})
      root.session = ""
      root.failed("The mail backend stopped unexpectedly. Restarting…")
      restartTimer.restart()
    }
  }

  Timer {
    id: restartTimer
    interval: 1500
    onTriggered: proc.running = true
  }
}
