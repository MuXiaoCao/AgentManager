#!/usr/bin/env swift
import Foundation
import CoreGraphics

@_silgen_name("CGSMainConnectionID")
func CGSMainConnectionID() -> Int32

@_silgen_name("CGSCopySpacesForWindows")
func CGSCopySpacesForWindows(_ conn: Int32, _ mask: Int32, _ windows: NSArray) -> NSArray?

@_silgen_name("CGSMoveWindowsToManagedSpace")
func CGSMoveWindowsToManagedSpace(_ conn: Int32, _ windows: NSArray, _ space: Int)

@_silgen_name("CGSGetActiveSpace")
func CGSGetActiveSpace(_ conn: Int32) -> Int

@_silgen_name("CGSCopyManagedDisplayForWindow")
func CGSCopyManagedDisplayForWindow(_ conn: Int32, _ wid: Int32) -> CFString?

@_silgen_name("CGSManagedDisplayGetCurrentSpace")
func CGSManagedDisplayGetCurrentSpace(_ conn: Int32, _ display: CFString) -> Int

func log(_ msg: String) { fputs("[move-to-space] \(msg)\n", stderr) }

guard CommandLine.arguments.count >= 3,
      let referencePid = Int32(CommandLine.arguments[1]),
      let targetPid = Int32(CommandLine.arguments[2]) else {
    log("usage: move-to-space <reference_pid> <target_pid>")
    exit(1)
}

let conn = CGSMainConnectionID()
let globalActiveSpace = CGSGetActiveSpace(conn)
log("conn=\(conn) globalActiveSpace=\(globalActiveSpace)")

guard let allWindows = CGWindowListCopyWindowInfo([.optionAll], kCGNullWindowID) as? [[String: Any]] else {
    log("ERROR: CGWindowListCopyWindowInfo returned nil"); exit(1)
}

// Step 1: find reference (AgentManager) window → determine target Space via TWO methods.
var destSpaceFromCopy: Int? = nil
var destSpaceFromManaged: Int? = nil
var refWid: Int32? = nil

for w in allWindows {
    guard let pid = w[kCGWindowOwnerPID as String] as? Int32, pid == referencePid,
          let wid = w[kCGWindowNumber as String] as? Int32,
          let layer = w[kCGWindowLayer as String] as? Int32, layer == 0
    else { continue }

    let widArray: NSArray = [wid as NSNumber]
    let spacesFromCopy = (CGSCopySpacesForWindows(conn, 0x7, widArray) as? [Int]) ?? []

    if let displayUUID = CGSCopyManagedDisplayForWindow(conn, wid) {
        let managedSpace = CGSManagedDisplayGetCurrentSpace(conn, displayUUID)
        log("ref wid=\(wid) spacesFromCopy=\(spacesFromCopy) displayUUID=\(displayUUID) managedCurrentSpace=\(managedSpace)")
        if destSpaceFromManaged == nil {
            destSpaceFromManaged = managedSpace
        }
    } else {
        log("ref wid=\(wid) spacesFromCopy=\(spacesFromCopy) (no display UUID)")
    }

    if !spacesFromCopy.isEmpty && destSpaceFromCopy == nil {
        destSpaceFromCopy = spacesFromCopy.first
        refWid = wid
    }
}

// Prefer the space from CGSCopySpacesForWindows (directly tied to the window).
// Fall back to managed display current space if Copy returned empty.
let targetSpace = destSpaceFromCopy ?? destSpaceFromManaged

guard let space = targetSpace else {
    log("ERROR: could not determine reference window's Space"); exit(1)
}
log("chosen targetSpace=\(space) (fromCopy=\(destSpaceFromCopy ?? -1) fromManaged=\(destSpaceFromManaged ?? -1))")

// Step 2: move target (iTerm) windows.
var moved = 0
var verified = 0
var failed = 0
for w in allWindows {
    guard let pid = w[kCGWindowOwnerPID as String] as? Int32, pid == targetPid,
          let wid = w[kCGWindowNumber as String] as? Int32,
          let layer = w[kCGWindowLayer as String] as? Int32, layer == 0
    else { continue }

    let widArray: NSArray = [wid as NSNumber]
    let before = (CGSCopySpacesForWindows(conn, 0x7, widArray) as? [Int]) ?? []

    if before.contains(space) {
        log("wid=\(wid) already on space \(space), skip")
        moved += 1; continue
    }

    CGSMoveWindowsToManagedSpace(conn, widArray, space)

    // Verify: is the window actually on the target space now?
    let after = (CGSCopySpacesForWindows(conn, 0x7, widArray) as? [Int]) ?? []
    if after.contains(space) {
        log("wid=\(wid) ✓ moved from=\(before) to=\(after)")
        verified += 1
    } else {
        log("wid=\(wid) ✗ FAILED from=\(before) still=\(after)")
        failed += 1
    }
    moved += 1
}

log("done: moved=\(moved) verified=\(verified) failed=\(failed)")
print("\(moved),\(verified),\(failed)")
