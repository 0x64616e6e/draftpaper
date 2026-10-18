// The equipment schedule (issue #49): what neofetch tells, as a drawing's schedule of equipment: REF,
// ITEM, DESCRIPTION rows for the machine, processor, graphics, memory, storage, system and desktop
// (`draftpaper machine`, polled by shell.qml), each display (live, from the compositor) and the sheet.
// Drawn by Schedule.qml, on the sheet; Mod+i brings it, then the processes, then neither.
import QtQuick

Schedule {
    id: equip
    readonly property var m: shell.machine
    title: "SCHEDULE OF EQUIPMENT"
    meta: String(theme.title.project || "").toUpperCase() + " · SHEET " + (edge.activeSpace ? edge.activeSpace.number : 1)
    columns: [{ name: "REF", mm: 9 }, { name: "ITEM", mm: 27 }, { name: "DESCRIPTION", mm: 0 }]
    noteCol: 2

    function uptime(s) {
        const d = Math.floor(s / 86400), h = Math.floor(s % 86400 / 3600), mi = Math.floor(s % 3600 / 60)
        return "up " + (d > 0 ? d + " d " + h + " h" : h > 0 ? h + " h " + mi + " min" : mi + " min")
    }
    rows: {
        if (!m) return []
        const r = []
        const add = (ref, item, text, note) => { if (text) r.push({ cells: [ref, item, text], note: note || "" }) }
        add("M1", "MACHINE", m.machine)
        if (m.cpu) add("M2", "PROCESSOR", m.cpu.model, m.cpu.cores + " cores, " + m.cpu.threads + " threads")
        ;(m.gpus || []).forEach((g, i) => add("M" + (3 + i), "GRAPHICS", g))
        const n = 3 + Math.max(1, (m.gpus || []).length)
        if (m.memory_gib) add("M" + n, "MEMORY", Math.round(m.memory_gib) + " GiB")
        if (m.storage) add("M" + (n + 1), "STORAGE", m.storage.disk,
            [m.storage.fs, m.storage.used_gb !== undefined ? Math.round(m.storage.used_gb) + " of " + Math.round(m.storage.size_gb) + " GB used" : ""].filter(x => x).join(" · "))
        add("S1", "SYSTEM", [m.system, m.kernel].filter(x => x).join(" · "), m.uptime_s ? uptime(m.uptime_s) : "")
        add("S2", "DESKTOP", "Draftpaper " + m.draftpaper + (m.quickshell ? " · Quickshell " + m.quickshell : ""))
        Compositor.outputs.forEach((o, i) => {
            const p = o.physical || [0, 0], inches = Math.hypot(p[0], p[1]) / 25.4
            const mode = o.mode || [Math.round(o.width * o.scale), Math.round(o.height * o.scale)]
            add("D" + (i + 1), "DISPLAY " + o.name,
                (inches > 0 ? inches.toFixed(1) + "″ · " : "") + mode[0] + " × " + mode[1] + " · scale " + o.scale,
                (o.px_per_mm * o.scale).toFixed(1) + " px/mm · " + o.px_per_mm.toFixed(1) + " logical px a mm"
                + (o.model && o.model !== "Unknown" ? " · " + o.model : ""))
        })
        const margin = out ? Math.round(out.margin / out.px_per_mm) : 6
        add("P1", "SHEET", theme.label + " · margin " + margin + " mm",
            (out ? out.zones.across + " × " + out.zones.down + " zones · " : "") + Fonts.mono)
        return r.map((row, i) => Object.assign(row, { heavyAbove: i > 0 && r[i - 1].cells[0][0] !== row.cells[0][0] }))
    }
}
