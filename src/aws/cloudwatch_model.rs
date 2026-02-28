/// CloudWatchメトリクスのデータポイント
#[derive(Debug, Clone, PartialEq)]
pub struct MetricDataPoint {
    pub timestamp: f64,
    pub value: f64,
}

/// メトリクス結果（ラベル付きデータポイント列）
#[derive(Debug, Clone, PartialEq)]
pub struct MetricResult {
    pub label: String,
    pub data_points: Vec<MetricDataPoint>,
}
