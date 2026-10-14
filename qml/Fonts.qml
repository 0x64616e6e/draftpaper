pragma Singleton
// The sheet's lettering face: IBM Plex Mono (fonts-ibm-plex, in Debian's contrib section) when installed,
// otherwise DejaVu Sans Mono, which every Debian has.
import QtQuick
import Quickshell

Singleton {
    readonly property string mono: Qt.fontFamilies().indexOf("IBM Plex Mono") >= 0 ? "IBM Plex Mono" : "DejaVu Sans Mono"
}
