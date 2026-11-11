/// Explicit rendering policy for a combined conversion.
public struct AnyDocMarkdownOptions: Sendable, Equatable {
  /// Superscript/subscript representation.
  public enum Scripts: Sendable { case plainText, latex }
  /// Quantitative chart representation.
  public enum Charts: Sendable { case tables, mermaidOrTable }
  /// Table and anchor portability.
  public enum Tables: Sendable { case gfm, portable }
  /// Embedded image representation.
  public enum Images: Sendable { case alternativeText, assetReferences }
  public let scripts: Scripts
  public let charts: Charts
  public let tables: Tables
  public let images: Images

  public init(
    scripts: Scripts = .plainText, charts: Charts = .tables,
    tables: Tables = .gfm, images: Images = .alternativeText
  ) {
    self.scripts = scripts; self.charts = charts; self.tables = tables; self.images = images
  }
  var flags: UInt32 {
    (scripts == .latex ? 1 : 0) | (charts == .mermaidOrTable ? 2 : 0)
      | (tables == .portable ? 4 : 0) | (images == .assetReferences ? 8 : 0)
  }
}

/// Source identity retained separately from translated user-facing notices.
public struct AnyDocSourceLocation: Sendable, Equatable {
  public let part: String
  public let sheet: String?
  public let object: String?
}

/// A stable adaptation or unavailable-content notice.
public struct AnyDocDiagnostic: Sendable, Equatable {
  public enum Severity: String, Sendable { case information, warning }
  public let code: String
  public let severity: Severity
  public let location: AnyDocSourceLocation?
  public let arguments: [String]
}

/// One parse yielding rendered Markdown and its original structured content.
public struct AnyDocConversion: Sendable, Equatable {
  public let markdown: String
  /// PDF uses the original Markdown-only engine and has no structured document.
  public let document: AnyDocDocument?
  public let diagnostics: [AnyDocDiagnostic]
  public var assets: [AnyDocDocument.Asset] { document?.assets ?? [] }
}

/// A source chart. Unsupported visual forms retain every available series value.
public struct AnyDocChart: Sendable, Equatable {
  public struct Series: Sendable, Equatable {
    public let name: String
    public let categories: [String]
    public let values: [String?]
    public let displayValues: [String?]
    public let sourceRanges: [String]
    /// Extra source dimensions retained by table fallbacks.
    public let additionalValues: [String: [String?]]
  }
  public let kind: String
  public let title: String
  public let grouping: String
  public let horizontal: Bool
  public let incompatibleAxes: Bool
  public let axisTitles: [String]
  public let series: [Series]
  public let location: AnyDocSourceLocation
}
