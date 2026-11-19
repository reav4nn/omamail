pragma ComponentBehavior: Bound

import QtQuick
import qs.Commons
import qs.Ui

Item {
  id: root
  required property var app
  enabled: !app.busy

  Column {
    anchors.centerIn: parent
    width: Math.min(parent.width - Style.space(48), Style.space(680))
    spacing: Style.space(12)

    Text {
      text: "Gmail accounts"
      color: Color.foreground
      font.family: Style.font.family
      font.pixelSize: Style.font.display
    }

    ListView {
      width: parent.width
      height: Math.min(contentHeight, root.height * 0.55)
      clip: true
      model: root.app.accounts
      delegate: Row {
        id: row
        required property var modelData
        width: ListView.view.width
        height: Style.space(42)
        spacing: Style.space(8)
        Button {
          width: row.width - reauth.width - remove.width - 2 * row.spacing
          leftAlign: true
          bordered: row.modelData.id === root.app.accountId
          text: row.modelData.email
          tooltipText: "Open " + row.modelData.email
          onClicked: {
            if (row.modelData.id === root.app.accountId && root.app.configured) root.app.backToMailbox()
            else root.app.changeAccount("select", {account: row.modelData.id}, true)
          }
        }
        Button { id: reauth; text: "Sign in again"; onClicked: root.app.loginForm(row.modelData.email) }
        Button {
          id: remove
          text: "Remove"
          onClicked: root.app.changeAccount("remove", {account: row.modelData.id}, row.modelData.id === root.app.accountId)
        }
      }
    }

    Text {
      width: parent.width
      visible: text.length > 0
      text: root.app.busy ? "Updating accounts…" : root.app.signInError
      textFormat: Text.PlainText
      wrapMode: Text.WordWrap
      color: root.app.busy ? Color.foreground : Color.urgent
      font.family: Style.font.family
      font.pixelSize: Style.font.bodySmall
    }
    Row {
      spacing: Style.space(8)
      Button { bordered: true; text: "Add account"; onClicked: root.app.loginForm("") }
      Button { text: "Back"; onClicked: root.app.backToMailbox() }
    }
  }
}
