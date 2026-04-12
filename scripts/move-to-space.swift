#!/usr/bin/env swift
// move-to-space — Moves all windows of <target_pid> to the same macOS Space
// as <reference_pid>'s window. This avoids relying on CGSGetActiveSpace which
// can return the wrong space if AppleEvents or app activation caused a space
// switch before this helper runs.
//
// Usage: move-to-space <reference_pid> <target_pid>
//   reference_pid = AgentManager's PID (determines the destination Space)
//   target_pid    = iTerm's PID (windows to move)

import Foundation
import CoreGraphics

// MARK: – Private CGS declarations (stable since macOS 10.10, used by yabai/Rectangle/Amethyst)

@_silgen_name("CGSMainConnectionID")
func CGSMainConnectionID() -> Int32

@_silgen_name("CGSAddWindowsToSpaces")
func CGSAddWindowsToSpaces(_ conn: Int32, _ windows: NSArray, _ spaces: NSArray) -> Int32

@_silgen_name("CGSRemoveWindowsFromSpaces")
func CGSRemoveWindowsFromSpaces(_ conn: Int32, _ windows: NSArray, _ spaces: NSArray) -> Int32

@_silgen_name("CGSCopySpacesForWindows")
func CGSCopySpacesForWindows(_ conn: Int32, _ mask: Int32, _ windows: NSArray) -> NSArray?

// MARK: – Main

guard CommandLine.arguments.count >= 3,
      let referencePid = Int32(CommandLine.arguments[1]),
      let targetPid = Int32(CommandLine.arguments[2]) else {
    fputs("usage: move-to-space <reference_pid> <target_pid>\n", stderr)
    exit(1)
}

let conn = CGSMainConnectionID()

guard let allWindows = CGWindowListCopyWindowInfo([.optionAll], kCGNullWindowID) as? [[String: Any]] else {
    fputs("failed to list windows\n", stderr)
    exit(1)
}

// Step 1: find the reference app's (AgentManager) window and determine its Space.
var destSpace: Int? = nil
for w in allWindows {
    guard let pid = w[kCGWindowOwnerPID as String] as? Int32, pid == referencePid,
          let wid = w[kCGWindowNumber as String] as? Int32,
          let layer = w[kCGWindowLayer as String] as? Int32, layer == 0  // normal windows only
    else { continue }

    let widArray: NSArray = [wid as NSNumber]
    if let spaces = CGSCopySpacesForWindows(conn, 0x7, widArray) {
        for s in spaces {
            if let spaceId = s as? Int {
                destSpace = spaceId
                break
            }
        }
    }
    if destSpace != nil { break }
}

guard let targetSpace = destSpace else {
    fputs("could not determine reference window's Space\n", stderr)
    exit(1)
}

let destSpaces: NSArray = [targetSpace as NSNumber]

// Step 2: move every target app's (iTerm) window to that Space.
var moved = 0
for w in allWindows {
    guard let pid = w[kCGWindowOwnerPID as String] as? Int32, pid == targetPid,
          let wid = w[kCGWindowNumber as String] as? Int32,
          let layer = w[kCGWindowLayer as String] as? Int32, layer == 0
    else { continue }

    let widArray: NSArray = [wid as NSNumber]

    if let currentSpaces = CGSCopySpacesForWindows(conn, 0x7, widArray), currentSpaces.count > 0 {
        // Check if already on the target space
        var alreadyThere = false
        for s in currentSpaces {
            if let spaceId = s as? Int, spaceId == targetSpace {
                alreadyThere = true
                break
            }
        }
        if alreadyThere { moved += 1; continue }
        CGSRemoveWindowsFromSpaces(conn, widArray, currentSpaces)
    }
    CGSAddWindowsToSpaces(conn, widArray, destSpaces)
    moved += 1
}

print("\(moved)")
