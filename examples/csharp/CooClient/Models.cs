using System.Text.Json.Serialization;

namespace CooClient;

/// 所有接口统一返回这个信封
public sealed record ApiEnvelope<T>(
    int Code,
    string Message,
    T? Data,
    string Timestamp
);

/// GET /api/v1/app/channels 的一项
public sealed record ChannelView(
    Guid Guid,
    [property: JsonPropertyName("tag_name")] string TagName,
    [property: JsonPropertyName("latest_version")] string LatestVersion,
    [property: JsonPropertyName("raw_sha256")] string RawSha256,
    [property: JsonPropertyName("raw_size")] long RawSize,
    [property: JsonPropertyName("is_default")] bool IsDefault
);

/// GET /api/v1/app/channels/{guid} 里的一条差分
public sealed record DiffView(
    [property: JsonPropertyName("base_sha256")] string BaseSha256,
    [property: JsonPropertyName("patch_sha256")] string PatchSha256,
    string? Algo,
    long Size
);

/// GET /api/v1/app/channels/{guid}
public sealed record UpdateView(
    Guid Guid,
    [property: JsonPropertyName("tag_name")] string TagName,
    string Version,
    [property: JsonPropertyName("raw_sha256")] string RawSha256,
    [property: JsonPropertyName("raw_size")] long RawSize,
    IReadOnlyList<DiffView> Diffs
);

/// GET /api/v1/app/announces 的一项（客户端只读，已过滤不可见的）
public sealed record AnnounceView(
    Guid Guid,
    string Title,
    string Content,
    [property: JsonPropertyName("starts_at")] DateTimeOffset? StartsAt,
    [property: JsonPropertyName("expires_at")] DateTimeOffset? ExpiresAt,
    [property: JsonPropertyName("created_at")] DateTimeOffset CreatedAt,
    [property: JsonPropertyName("updated_at")] DateTimeOffset UpdatedAt
);
