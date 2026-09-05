using CooClient;

// 用法：
//   dotnet run -- <baseUrl> <appId> [本地版本的资源 sha256]
//   dotnet run -- http://127.0.0.1:8081 6f1c2f4e-9a3b-4d5e-8c7a-1b2d3e4f5a6b aa11
//
// 环境变量 COO_BASE_URL / COO_APP_ID 也能替代前两个参数。

var baseUrl = args.ElementAtOrDefault(0) ?? Environment.GetEnvironmentVariable("COO_BASE_URL") ?? "http://127.0.0.1:8081";
var appIdArg = args.ElementAtOrDefault(1) ?? Environment.GetEnvironmentVariable("COO_APP_ID");
var localSha256 = args.ElementAtOrDefault(2);

if (!Guid.TryParse(appIdArg, out var appId))
{
    Console.Error.WriteLine("缺少 app id：dotnet run -- <baseUrl> <appId> [localSha256]");
    return 1;
}

var client = new CooServiceClient(baseUrl, appId);

try
{
    // 1. 取通道列表。服务端推荐 is_default，没有就取第一个。
    var channels = await client.GetChannelsAsync();
    if (channels.Count == 0)
    {
        Console.WriteLine("该 app 没有任何发版通道");
        return 0;
    }

    var chosen = channels.FirstOrDefault(c => c.IsDefault) ?? channels[0];
    Console.WriteLine($"通道列表（共 {channels.Count}）：");
    foreach (var channel in channels)
    {
        var mark = channel.Guid == chosen.Guid ? " ← 选中" : "";
        Console.WriteLine($"  {channel.TagName,-10} {channel.LatestVersion,-12} default={channel.IsDefault}{mark}");
    }

    // 2. 按本地存的 guid 取更新信息（真实客户端这里读自己的配置文件）
    var update = await client.GetUpdateAsync(chosen.Guid);
    Console.WriteLine($"\n{update.TagName} 最新版本 {update.Version}，raw 包 {update.RawSha256}（{update.RawSize} 字节）");

    // 3. 拉取公告（服务端只回当前可见的）
    var announces = await client.GetAnnouncesAsync();
    Console.WriteLine($"公告（共 {announces.Count}）：");
    foreach (var announce in announces)
        Console.WriteLine($"  [{announce.Title}] {announce.Content}");

    // 4. 拿本地版本的 sha256 去 diffs 里比对：命中就下补丁，否则下完整包
    var diff = localSha256 is null
        ? null
        : update.Diffs.FirstOrDefault(d => d.BaseSha256.Equals(localSha256, StringComparison.OrdinalIgnoreCase));

    string downloadSha;
    if (diff is not null)
    {
        Console.WriteLine($"命中差分：{diff.BaseSha256} + {diff.PatchSha256}（{diff.Algo ?? "未知算法"}，{diff.Size} 字节）");
        downloadSha = diff.PatchSha256;
    }
    else
    {
        Console.WriteLine(localSha256 is null ? "未提供本地 sha256，按完整包处理" : $"本地 sha256 {localSha256} 没有对应差分，下完整包");
        downloadSha = update.RawSha256;
    }

    // 5. 下载：GET /api/v1/resources/{sha}，服务端 302 重定向到公开存储池（HttpClient 自动跟随）；
    //    无公开池时 503（服务整体不可下载）。
    Console.WriteLine($"下载：GET {baseUrl}/api/v1/resources/{downloadSha}");
    try
    {
        using var body = await client.DownloadAsync(downloadSha);
        Console.WriteLine($"302 重定向后收到 {body.Length} 字节 ✓");
    }
    catch (HttpRequestException e)
    {
        Console.WriteLine($"下载不可用：{e.Message}");
    }
    return 0;
}
catch (HttpRequestException e)
{
    Console.Error.WriteLine($"请求失败：{e.Message}");
    return 2;
}
