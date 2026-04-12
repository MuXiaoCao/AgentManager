#!/usr/bin/env swift
// move-to-space — Moves all windows belonging to the given PIDs to the
// current macOS Space (virtual desktop). Uses private CoreGraphics APIs
// that have been stable since macOS 10.10 and are relied upon by yabai,
// Rectangle, Amethyst, and other tiling window managers.
//
// Usage: move-to-space <pid> [<pid> ...]

import Foundation
import CoreGraphics

// MARK: – Private CGS declarations

@_silgen_name("CGSMainConnectionID")
func CGSMainConnectionID() -> Int32

@_silgen_name("CGSGetActiveSpace")
func CGSGetActiveSpace(_ conn: Int32) -> Int

@_silgen_name("CGSAddWindowsToSpaces")
func CGSAddWindowsToSpaces(_ conn: Int32, _ windows: NSArray, _ spaces: NSArray) -> Int32

@_silgen_name("CGSRemoveWindowsFromSpaces")
func CGSRemoveWindowsFromSpaces(_ conn: Int32, _ windows: NSArray, _ spaces: NSArray) -> Int32

@_silgen_name("CGSCopySpacesForWindows")
func CGSCopySpacesForWindows(_ conn: Int32, _ mask: Int32, _ windows: NSArray) -> NSArray?

// MARK: – Main

let targetPids: Set<Int32> = Set(CommandLine.arguments.dropFirst().compactMap { Int32($0) })
guard !targetPids.isEmpty else {
    fputs("usage: move-to-space <pid> [<pid> ...]\n", stderr)
    exit(1)
}

let conn = CGSMainConnectionID()
let activeSpace = CGSGetActiveSpace(conn)
let destSpaces: NSArray = [activeSpace as NSNumber]

guard let windowList = CGWindowListCopyWindowInfo([.optionAll], kCGNullWindowID) as? [[String: Any]] else {
    fputs("failed to list windows\n", stderr)
    exit(1)
}

var moved = 0
for w in windowList {
    guard let pid = w[kCGWindowOwnerPID as String] as? Int32,
          targetPids.contains(pid),
          let wid = w[kCGWindowNumber as String] as? Int32 else { continue }

    let widArray: NSArray = [wid as NSNumber]

    // Get the spaces this window currently lives on.
    if let currentSpaces = CGSCopySpacesForWindows(conn, 0x7, widArray), currentSpaces.count > 0 {
        CGSRemoveWindowsFromSpaces(conn, widArray, currentSpaces)
    }
    CGSAddWindowsToSpaces(conn, widArray, destSpaces)
    moved += 1
}

print("\(moved)")
