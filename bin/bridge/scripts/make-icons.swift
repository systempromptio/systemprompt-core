#!/usr/bin/env swift
//
// Generates two PNG assets for the macOS bundle:
//
//   bin/bridge/assets/window-icon-1024.png   Dock icon — 1024×1024, brand
//                                            squircle with subtle drop shadow
//                                            and chevron mark, ready for
//                                            iconutil.
//
//   bin/bridge/assets/tray-icon.png          Menu bar icon — 44×44 monochrome
//                                            template (black on transparent),
//                                            sized for retina display and
//                                            consumed by the tray-icon crate.
//
// Usage: swift bin/bridge/scripts/make-icons.swift <out-dir>

import AppKit
import Foundation

guard CommandLine.arguments.count == 2 else {
    FileHandle.standardError.write(Data("usage: make-icons.swift <out-dir>\n".utf8))
    exit(64)
}
let outDir = CommandLine.arguments[1]

func writePNG(_ rep: NSBitmapImageRep, to path: String) throws {
    guard let data = rep.representation(using: .png, properties: [:]) else {
        throw NSError(domain: "make-icons", code: 1, userInfo: [NSLocalizedDescriptionKey: "png encode failed"])
    }
    try data.write(to: URL(fileURLWithPath: path), options: .atomic)
}

func newCanvas(_ side: Int) -> (NSBitmapImageRep, CGContext) {
    let rep = NSBitmapImageRep(
        bitmapDataPlanes: nil,
        pixelsWide: side,
        pixelsHigh: side,
        bitsPerSample: 8,
        samplesPerPixel: 4,
        hasAlpha: true,
        isPlanar: false,
        colorSpaceName: .deviceRGB,
        bytesPerRow: 0,
        bitsPerPixel: 32
    )!
    let gctx = NSGraphicsContext(bitmapImageRep: rep)!
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = gctx
    return (rep, gctx.cgContext)
}

func squirclePath(in rect: CGRect, radius: CGFloat) -> CGPath {
    let path = CGMutablePath()
    let r = min(radius, min(rect.width, rect.height) / 2)
    let k = r * 0.55228
    let x = rect.minX, y = rect.minY, w = rect.width, h = rect.height
    path.move(to: CGPoint(x: x + r, y: y))
    path.addLine(to: CGPoint(x: x + w - r, y: y))
    path.addCurve(
        to: CGPoint(x: x + w, y: y + r),
        control1: CGPoint(x: x + w - r + k, y: y),
        control2: CGPoint(x: x + w, y: y + r - k))
    path.addLine(to: CGPoint(x: x + w, y: y + h - r))
    path.addCurve(
        to: CGPoint(x: x + w - r, y: y + h),
        control1: CGPoint(x: x + w, y: y + h - r + k),
        control2: CGPoint(x: x + w - r + k, y: y + h))
    path.addLine(to: CGPoint(x: x + r, y: y + h))
    path.addCurve(
        to: CGPoint(x: x, y: y + h - r),
        control1: CGPoint(x: x + r - k, y: y + h),
        control2: CGPoint(x: x, y: y + h - r + k))
    path.addLine(to: CGPoint(x: x, y: y + r))
    path.addCurve(
        to: CGPoint(x: x + r, y: y),
        control1: CGPoint(x: x, y: y + r - k),
        control2: CGPoint(x: x + r - k, y: y))
    path.closeSubpath()
    return path
}

func makeDockIcon() -> NSBitmapImageRep {
    let canvas: CGFloat = 1024
    let safe: CGFloat = 824
    let pad: CGFloat = (canvas - safe) / 2
    let cornerRadius: CGFloat = 185.4

    let (rep, cg) = newCanvas(Int(canvas))
    cg.clear(CGRect(x: 0, y: 0, width: canvas, height: canvas))

    let body = CGRect(x: pad, y: pad, width: safe, height: safe)
    let path = squirclePath(in: body, radius: cornerRadius)

    cg.saveGState()
    cg.setShadow(
        offset: CGSize(width: 0, height: -8),
        blur: 28,
        color: NSColor.black.withAlphaComponent(0.30).cgColor)
    cg.addPath(path)

    let gradColors = [
        NSColor(red: 0xFF/255.0, green: 0xA8/255.0, blue: 0x4A/255.0, alpha: 1).cgColor,
        NSColor(red: 0xF7/255.0, green: 0x99/255.0, blue: 0x38/255.0, alpha: 1).cgColor,
        NSColor(red: 0xE0/255.0, green: 0x7C/255.0, blue: 0x1F/255.0, alpha: 1).cgColor,
    ] as CFArray
    let cs = CGColorSpaceCreateDeviceRGB()
    let gradient = CGGradient(colorsSpace: cs, colors: gradColors, locations: [0.0, 0.55, 1.0])!

    cg.clip()
    cg.drawLinearGradient(
        gradient,
        start: CGPoint(x: pad, y: pad + safe),
        end: CGPoint(x: pad, y: pad),
        options: [])
    cg.restoreGState()

    cg.saveGState()
    cg.addPath(path)
    cg.clip()
    let highlightColors = [
        NSColor.white.withAlphaComponent(0.18).cgColor,
        NSColor.white.withAlphaComponent(0.0).cgColor,
    ] as CFArray
    let highlight = CGGradient(colorsSpace: cs, colors: highlightColors, locations: [0.0, 0.7])!
    cg.drawLinearGradient(
        highlight,
        start: CGPoint(x: pad, y: pad + safe),
        end: CGPoint(x: pad, y: pad + safe * 0.45),
        options: [])
    cg.restoreGState()

    let scale = safe / 32.0
    let ox = pad
    let oy = pad
    func p(_ sx: CGFloat, _ sy: CGFloat) -> CGPoint {
        CGPoint(x: ox + sx * scale, y: oy + (32 - sy) * scale)
    }

    cg.saveGState()
    cg.setStrokeColor(NSColor.white.cgColor)
    cg.setLineWidth(2.6 * scale)
    cg.setLineCap(.round)
    cg.setLineJoin(.round)

    cg.move(to: p(14, 10))
    cg.addLine(to: p(8, 16))
    cg.addLine(to: p(14, 22))
    cg.strokePath()

    cg.move(to: p(24, 10))
    cg.addLine(to: p(18, 22))
    cg.strokePath()
    cg.restoreGState()

    NSGraphicsContext.restoreGraphicsState()
    return rep
}

func makeTrayIcon() -> NSBitmapImageRep {
    let canvas: CGFloat = 44
    let (rep, cg) = newCanvas(Int(canvas))
    cg.clear(CGRect(x: 0, y: 0, width: canvas, height: canvas))

    let scale = canvas / 32.0
    func p(_ sx: CGFloat, _ sy: CGFloat) -> CGPoint {
        CGPoint(x: sx * scale, y: (32 - sy) * scale)
    }

    cg.setStrokeColor(NSColor.black.cgColor)
    cg.setLineWidth(3.0 * scale)
    cg.setLineCap(.round)
    cg.setLineJoin(.round)

    cg.move(to: p(14, 10))
    cg.addLine(to: p(8, 16))
    cg.addLine(to: p(14, 22))
    cg.strokePath()

    cg.move(to: p(24, 10))
    cg.addLine(to: p(18, 22))
    cg.strokePath()

    NSGraphicsContext.restoreGraphicsState()
    return rep
}

do {
    try writePNG(makeDockIcon(), to: "\(outDir)/window-icon-1024.png")
    try writePNG(makeTrayIcon(), to: "\(outDir)/tray-icon.png")
    print("wrote \(outDir)/window-icon-1024.png and \(outDir)/tray-icon.png")
} catch {
    FileHandle.standardError.write(Data("error: \(error)\n".utf8))
    exit(1)
}
