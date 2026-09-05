using System.Net.Http.Headers;
using System.Net.Http.Json;
using System.Text.Json;

namespace CooClient;

/// CooService 客户端接口的极简封装
public sealed class CooServiceClient
{
    private static readonly JsonSerializerOptions Json = new(JsonSerializerDefaults.Web);

    private readonly HttpClient _http;

    /// <param name="baseUrl">服务地址，如 http://127.0.0.1:8081</param>
    /// <param name="appId">客户端对应的 app id，会作为 X-App-Id 头发给服务端</param>
    public CooServiceClient(string baseUrl, Guid appId)
    {
        _http = new HttpClient { BaseAddress = new Uri(baseUrl.TrimEnd('/')) };
        _http.DefaultRequestHeaders.Add("X-App-Id", appId.ToString());
    }

    /// 列出 app 的全部通道。默认通道排最前。
    public async Task<IReadOnlyList<ChannelView>> GetChannelsAsync(CancellationToken ct = default)
    {
        var envelope = await GetAsync<ApiEnvelope<List<ChannelView>>>("/api/v1/app/channels", ct);
        return envelope?.Data ?? [];
    }

    /// 按通道 guid 取更新信息。
    public async Task<UpdateView> GetUpdateAsync(Guid channelGuid, CancellationToken ct = default)
    {
        var envelope = await GetAsync<ApiEnvelope<UpdateView>>($"/api/v1/app/channels/{channelGuid}", ct);
        return envelope?.Data
               ?? throw new InvalidOperationException($"channel {channelGuid} 没有返回数据");
    }

    /// 拉取当前可见的公告（起点已到、终点未过的，NULL 边不限）。
    public async Task<IReadOnlyList<AnnounceView>> GetAnnouncesAsync(CancellationToken ct = default)
    {
        var envelope = await GetAsync<ApiEnvelope<List<AnnounceView>>>("/api/v1/app/announces", ct);
        return envelope?.Data ?? [];
    }

    /// 下载资源内容：服务端只 302 重定向到公开存储池（不代理流转发），
    /// HttpClient 默认自动跟随重定向；无可用公开池时服务端返回 503。
    public async Task<Stream> DownloadAsync(string sha256, CancellationToken ct = default)
    {
        var response = await _http.GetAsync(
            $"/api/v1/resources/{sha256}",
            HttpCompletionOption.ResponseHeadersRead,
            ct);

        if (!response.IsSuccessStatusCode)
        {
            throw new HttpRequestException($"下载失败：{(int)response.StatusCode}");
        }
        return await response.Content.ReadAsStreamAsync(ct);
    }

    private async Task<T?> GetAsync<T>(string path, CancellationToken ct)
    {
        using var response = await _http.GetAsync(path, ct);

        var body = await response.Content.ReadAsStringAsync(ct);
        if (!response.IsSuccessStatusCode)
        {
            // 失败也是同一套信封，code / message 可直接读
            var error = JsonSerializer.Deserialize<ApiEnvelope<object>>(body, Json);
            throw new HttpRequestException($"{(int)response.StatusCode} {error?.Message ?? body}");
        }

        return JsonSerializer.Deserialize<T>(body, Json);
    }
}
