import Foundation
import XCTest
@testable import AnyDocSwift

final class SpreadsheetFollowupTests: XCTestCase, @unchecked Sendable {
  private let options = AnyDocMarkdownOptions(
    scripts: .latex, charts: .mermaidOrTable, tables: .portable, images: .assetReferences)

  func testSpreadsheetFollowupsThroughPublicCombinedAPI() async throws {
    let result = try await AnyDocConverter().convert(
      from: fixtureData("fidelity/spreadsheet-followups.xlsx"), format: .xlsx, options: options)
    XCTAssertTrue(result.markdown.contains("$1,618.50"))
    XCTAssertTrue(result.markdown.contains("1618.5"))
    XCTAssertTrue(result.markdown.contains("2019-01-01"))
    XCTAssertTrue(result.markdown.contains("25%"))
    XCTAssertTrue(result.markdown.contains("Formula result unavailable"))
    XCTAssertFalse(result.markdown.contains("[Formula result unavailable]"))
    XCTAssertTrue(result.markdown.contains("example.org/guide_"))
    XCTAssertTrue(result.markdown.contains("#details"))
    XCTAssertFalse(result.markdown.contains("javascript:"))
    XCTAssertFalse(result.markdown.contains("<a "))
    let codes = Set(result.diagnostics.map(\.code))
    for code in [
      "spreadsheet.formulaResultUnavailable", "spreadsheet.sheetUnavailable",
      "spreadsheet.linkUnavailable", "anchor.targetUnavailable", "chart.cachedData",
      "chart.tableFallback",
    ] {
      XCTAssertTrue(codes.contains(code), "Missing diagnostic: \(code)")
    }
    let charts = try XCTUnwrap(result.document).blocks.compactMap { block -> AnyDocChart? in
      if case .chart(let chart) = block { return chart }; return nil
    }
    XCTAssertEqual(charts.count, 2)
    XCTAssertEqual(charts[0].location.sheet, "Date chart")
    XCTAssertEqual(charts[0].series[0].categories, ["2019-01-01", "2019-01-02", "2019-01-03"])
    XCTAssertEqual(charts[0].series[0].values, ["71", "53", "58"])
    XCTAssertEqual(charts[1].series[0].values, ["0.25", "0.5", "0.75"])
    XCTAssertEqual(charts[1].series[0].displayValues, ["25%", "50%", "75%"])
    XCTAssertEqual(result.markdown.components(separatedBy: "```mermaid").count - 1, 1)
  }

  func testCachedChartDatesRespectThe1904WorkbookEpoch() async throws {
    let result = try await AnyDocConverter().convert(
      from: fixtureData("fidelity/spreadsheet-followups-1904.xlsx"), format: .xlsx, options: options
    )
    let charts = try XCTUnwrap(result.document).blocks.compactMap { block -> AnyDocChart? in
      if case .chart(let chart) = block { return chart }; return nil
    }
    XCTAssertEqual(charts[0].series[0].categories, ["1904-01-01", "1904-01-02", "1904-01-03"])
  }
}
