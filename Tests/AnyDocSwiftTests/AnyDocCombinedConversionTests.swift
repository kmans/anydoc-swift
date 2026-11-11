import Foundation
import XCTest
@testable import AnyDocSwift

final class AnyDocCombinedConversionTests: XCTestCase, @unchecked Sendable {
  private let options = AnyDocMarkdownOptions(
    scripts: .latex, charts: .mermaidOrTable, tables: .portable, images: .assetReferences)

  func testRealHandbookPreservesMeaningCodeAndInlineAssets() async throws {
    let result = try await AnyDocConverter().convert(
      from: fixtureData("fidelity/handbook.docx"), format: .docx, options: options)
    XCTAssertTrue(result.markdown.contains("$10^{3}$"))
    XCTAssertTrue(result.markdown.contains("$\\mathrm{H}_{2}\\mathrm{O}$"))
    XCTAssertTrue(result.markdown.contains("```\nmoor --boat Aurora"))
    let reference = try XCTUnwrap(result.assets.first?.markdownReference)
    XCTAssertTrue(result.markdown.contains("](" + reference + ") The east quay"))
    XCTAssertEqual(result.assets.count, 2)
    XCTAssertFalse(result.markdown.contains("<a "))
    XCTAssertFalse(result.markdown.contains("<br>"))
  }

  func testRealWorkbookExposesAllChartsAndAnHonestFallback() async throws {
    let result = try await AnyDocConverter().convert(
      from: fixtureData("fidelity/workbook.xlsx"), format: .xlsx, options: options)
    let charts = try XCTUnwrap(result.document).blocks.compactMap { block -> AnyDocChart? in
      if case .chart(let chart) = block { return chart }; return nil
    }
    XCTAssertEqual(charts.count, 3)
    XCTAssertEqual(charts.first?.series.map(\.name), ["Jan", "Jun"])
    XCTAssertTrue(result.markdown.contains("## Charts"))
    XCTAssertTrue(result.markdown.contains("**Jan**") && result.markdown.contains("**Jun**"))
    XCTAssertTrue(result.markdown.contains("-0.0577489687684149"))
    let notice = try XCTUnwrap(
      result.diagnostics.first {
        $0.code == "chart.tableFallback" && $0.location?.object == "Share of the total"
      })
    XCTAssertEqual(notice.location?.sheet, "Charts")
    XCTAssertEqual(notice.location?.object, "Share of the total")
  }

  func testCombinedConversionUsesOneOwnedNativeInvocation() async throws {
    let bridge = FakeNativeBridge(responseProvider: { _ in
      .document(manifest: Self.manifest("hello"), assets: [])
    })
    let converter = AnyDocConverter(adapter: bridge.makeAdapter(), enqueue: { $0() })
    let result = try await converter.convert(from: Data([1]), format: .docx, options: options)
    XCTAssertEqual(result.markdown, "hello")
    XCTAssertEqual(bridge.invocations.map(\.operation), [.combined])
    XCTAssertEqual(bridge.freeCount, 1)
  }

  func testCombinedOutputLimitRejectsOversizedMarkdownAndFreesResult() async throws {
    let bridge = FakeNativeBridge(responseProvider: { _ in
      .document(
        manifest: Self.manifest("too long").replacingOccurrences(
          of: "\"assets\":[]",
          with:
            "\"assets\":[{\"id\":0,\"mediaType\":\"image/png\",\"originPart\":\"image.png\",\"byteLength\":2}]"
        ), assets: [[1, 2]])
    })
    let converter = AnyDocConverter(
      limits: .init(maximumInputBytes: 10, maximumOutputBytes: 3), adapter: bridge.makeAdapter(),
      enqueue: { $0() })
    do { _ = try await converter.convert(from: Data([1])); XCTFail("Expected output limit") } catch
    { XCTAssertEqual(error as? AnyDocConversionError, .outputTooLarge(maximumBytes: 3)) }
    XCTAssertEqual(bridge.freeCount, 1)
    XCTAssertEqual(
      bridge.copyCount, 1, "Oversized Markdown must be rejected before asset bytes are copied")
  }

  func testCombinedInputLimitPrecedesAnyNativeInvocation() async throws {
    let bridge = FakeNativeBridge()
    let converter = AnyDocConverter(
      limits: .init(maximumInputBytes: 1, maximumOutputBytes: 10), adapter: bridge.makeAdapter(),
      enqueue: { $0() })
    do {
      _ = try await converter.convert(from: Data([1, 2])); XCTFail("Expected input limit")
    } catch {
      XCTAssertEqual(
        error as? AnyDocConversionError, .inputTooLarge(actualBytes: 2, maximumBytes: 1))
    }
    XCTAssertTrue(bridge.invocations.isEmpty)
  }

  func testMissingCombinedMarkdownIsAnInvariantFailure() async throws {
    let bridge = FakeNativeBridge(responseProvider: { _ in
      .document(manifest: Self.manifest(nil), assets: [])
    })
    do {
      _ = try await AnyDocConverter(adapter: bridge.makeAdapter(), enqueue: { $0() }).convert(
        from: Data([1]));
      XCTFail("Expected invalid transport")
    } catch {
      guard case .bridgeFailure = error as? AnyDocConversionError else {
        return XCTFail("Unexpected error: \(error)")
      }
    }
    XCTAssertEqual(bridge.freeCount, 1)
  }

  func testUnknownDiagnosticSeverityIsRejected() throws {
    let manifest = Self.manifest("hello").replacingOccurrences(
      of: "\"diagnostics\":[]",
      with:
        "\"diagnostics\":[{\"code\":\"chart.tableFallback\",\"severity\":\"future\",\"location\":null,\"arguments\":[]}]"
    )
    XCTAssertThrowsError(
      try AnyDocDocumentDecoder.decode(manifest: Data(manifest.utf8), assetBytes: []))
  }

  private static func manifest(_ markdown: String?) -> String {
    let value = markdown.map { "\"\($0)\"" } ?? "null"
    return
      "{\"schemaVersion\":2,\"markdown\":\(value),\"diagnostics\":[],\"blocks\":[],\"notes\":[],\"assets\":[]}"
  }
}

extension AnyDocCombinedConversionTests {
  func testGeneratedReferencesAreUniqueAndDoNotReplaceCodeExamples() async throws {
    let data = try fixtureData("fidelity/literal-image-references.docx")
    let first = try await AnyDocConverter().convert(from: data, format: .docx, options: options)
    let second = try await AnyDocConverter().convert(from: data, format: .docx, options: options)
    XCTAssertNotEqual(first.assets.first?.markdownReference, second.assets.first?.markdownReference)
    let literal = "![literal](anydoc-asset:0) ![literal](telescopo-import-image:image-0.png)"
    XCTAssertTrue(first.markdown.contains(literal))
    let reference = try XCTUnwrap(first.assets.first?.markdownReference)
    XCTAssertTrue(first.markdown.contains("](" + reference + ")"))
  }
}

extension AnyDocCombinedConversionTests {
  func testMissingRequestedAssetReferenceFailsBeforeCopyingImageBytes() async throws {
    let manifest = Self.manifest("hello").replacingOccurrences(
      of: "\"assets\":[]",
      with:
        "\"assets\":[{\"id\":0,\"mediaType\":\"image/png\",\"originPart\":\"image.png\",\"byteLength\":1}]"
    )
    let bridge = FakeNativeBridge(responseProvider: { _ in
      .document(manifest: manifest, assets: [[1]])
    })
    do {
      _ = try await AnyDocConverter(adapter: bridge.makeAdapter(), enqueue: { $0() }).convert(
        from: Data([1]), options: options)
      XCTFail("Expected incomplete reference transport to fail")
    } catch {
      guard case .bridgeFailure = error as? AnyDocConversionError else {
        return XCTFail("Unexpected error: \(error)")
      }
    }
    XCTAssertEqual(bridge.copyCount, 1)
    XCTAssertEqual(bridge.freeCount, 1)
  }
}
