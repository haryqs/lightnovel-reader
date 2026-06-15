using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.IO.Compression;
using System.Net;
using System.Security.Cryptography;
using System.Text;

namespace LightNovelReaderInstaller
{
    internal static partial class InstallerConfig
    {
    }

    internal sealed class Options
    {
        public string Url = InstallerConfig.DefaultDownloadUrl;
        public string Sha256 = InstallerConfig.DefaultSha256;
        public string InstallDir = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
            "Programs",
            "LightNovelReader");
        public bool NoLaunch;
        public bool NoShortcuts;
        public bool Quiet;
    }

    internal static class Program
    {
        private const string AppExeName = "reader.exe";

        private static int Main(string[] args)
        {
            try
            {
                ServicePointManager.SecurityProtocol |= SecurityProtocolType.Tls12;
                var options = ParseArgs(args);
                if (String.IsNullOrWhiteSpace(options.Url))
                {
                    throw new InvalidOperationException(
                        "No package URL is configured. Rebuild this installer with -DownloadUrl, or run with /url <zip-url>.");
                }

                Log("LightNovel Reader Setup");
                Log("Version: " + InstallerConfig.DefaultVersion);
                Log("Install dir: " + options.InstallDir);

                Directory.CreateDirectory(options.InstallDir);
                var workDir = Path.Combine(Path.GetTempPath(), "LightNovelReaderSetup-" + Guid.NewGuid().ToString("N"));
                Directory.CreateDirectory(workDir);

                try
                {
                    var zipPath = Path.Combine(workDir, "package.zip");
                    FetchPackage(options.Url, zipPath);
                    VerifyPackage(zipPath, options.Sha256);

                    var extractDir = Path.Combine(workDir, "extract");
                    ZipFile.ExtractToDirectory(zipPath, extractDir);
                    var sourceRoot = FindPackageRoot(extractDir);
                    InstallPackage(sourceRoot, options.InstallDir);
                    if (!options.NoShortcuts)
                    {
                        WriteLaunchers(options.InstallDir);
                    }

                    var appPath = Path.Combine(options.InstallDir, "App", AppExeName);
                    Log("Installed: " + appPath);
                    if (!options.NoLaunch)
                    {
                        Process.Start(new ProcessStartInfo
                        {
                            FileName = appPath,
                            WorkingDirectory = Path.GetDirectoryName(appPath),
                            UseShellExecute = true
                        });
                    }
                }
                finally
                {
                    TryDelete(workDir);
                }

                Log("Setup complete.");
                if (!options.Quiet)
                {
                    Log("Press Enter to close.");
                    Console.ReadLine();
                }
                return 0;
            }
            catch (Exception ex)
            {
                Console.Error.WriteLine("Setup failed:");
                Console.Error.WriteLine(ex.Message);
                Console.Error.WriteLine();
                Console.Error.WriteLine(ex.ToString());
                Console.Error.WriteLine();
                Console.Error.WriteLine("Press Enter to close.");
                Console.ReadLine();
                return 1;
            }
        }

        private static Options ParseArgs(string[] args)
        {
            var options = new Options();
            for (var i = 0; i < args.Length; i++)
            {
                var key = NormalizeKey(args[i]);
                if (key == "url")
                {
                    options.Url = RequireValue(args, ref i, key);
                }
                else if (key == "sha256")
                {
                    options.Sha256 = RequireValue(args, ref i, key);
                }
                else if (key == "install-dir")
                {
                    options.InstallDir = Path.GetFullPath(RequireValue(args, ref i, key));
                }
                else if (key == "no-launch")
                {
                    options.NoLaunch = true;
                }
                else if (key == "no-shortcuts")
                {
                    options.NoShortcuts = true;
                }
                else if (key == "quiet")
                {
                    options.Quiet = true;
                }
                else if (key == "help" || key == "?")
                {
                    PrintUsageAndExit();
                }
                else
                {
                    throw new ArgumentException("Unknown option: " + args[i]);
                }
            }
            return options;
        }

        private static string NormalizeKey(string raw)
        {
            return raw.TrimStart('-', '/').ToLowerInvariant();
        }

        private static string RequireValue(string[] args, ref int index, string key)
        {
            if (index + 1 >= args.Length)
            {
                throw new ArgumentException("Missing value for /" + key);
            }
            index++;
            return args[index];
        }

        private static void PrintUsageAndExit()
        {
            Console.WriteLine("LightNovelReaderSetup.exe [/url <zip-url>] [/sha256 <hex>] [/install-dir <dir>] [/no-launch] [/no-shortcuts] [/quiet]");
            Environment.Exit(0);
        }

        private static void FetchPackage(string source, string destination)
        {
            Log("Fetching package...");
            if (IsLocalPackage(source))
            {
                var localPath = ResolveLocalPath(source);
                File.Copy(localPath, destination, true);
                return;
            }

            using (var client = new WebClient())
            {
                client.Headers.Add("User-Agent", "LightNovelReaderSetup/" + InstallerConfig.DefaultVersion);
                client.DownloadFile(source, destination);
            }
        }

        private static bool IsLocalPackage(string source)
        {
            if (File.Exists(source)) return true;
            Uri uri;
            return Uri.TryCreate(source, UriKind.Absolute, out uri) && uri.IsFile;
        }

        private static string ResolveLocalPath(string source)
        {
            if (File.Exists(source)) return Path.GetFullPath(source);
            var uri = new Uri(source);
            if (!uri.IsFile) throw new FileNotFoundException("Package file not found.", source);
            return uri.LocalPath;
        }

        private static void VerifyPackage(string zipPath, string expectedSha256)
        {
            if (String.IsNullOrWhiteSpace(expectedSha256)) return;
            Log("Verifying SHA-256...");
            var actual = ComputeSha256(zipPath);
            if (!String.Equals(actual, expectedSha256.Trim(), StringComparison.OrdinalIgnoreCase))
            {
                throw new InvalidOperationException(
                    "Package SHA-256 mismatch. Expected " + expectedSha256 + ", got " + actual + ".");
            }
        }

        private static string ComputeSha256(string path)
        {
            using (var stream = File.OpenRead(path))
            using (var sha = SHA256.Create())
            {
                var hash = sha.ComputeHash(stream);
                var sb = new StringBuilder(hash.Length * 2);
                foreach (var b in hash)
                {
                    sb.Append(b.ToString("x2"));
                }
                return sb.ToString();
            }
        }

        private static string FindPackageRoot(string extractDir)
        {
            var directExe = Path.Combine(extractDir, AppExeName);
            if (File.Exists(directExe)) return extractDir;

            var candidates = Directory.GetFiles(extractDir, AppExeName, SearchOption.AllDirectories);
            if (candidates.Length == 0)
            {
                throw new FileNotFoundException("The downloaded package does not contain " + AppExeName + ".");
            }
            Array.Sort(candidates, StringComparer.OrdinalIgnoreCase);
            return Path.GetDirectoryName(candidates[0]);
        }

        private static void InstallPackage(string sourceRoot, string installDir)
        {
            Log("Installing...");
            var appDir = Path.Combine(installDir, "App");
            var backupDir = Path.Combine(installDir, "App.old");

            TryDelete(backupDir);
            if (Directory.Exists(appDir))
            {
                Directory.Move(appDir, backupDir);
            }

            try
            {
                CopyDirectory(sourceRoot, appDir);
                TryDelete(backupDir);
            }
            catch
            {
                TryDelete(appDir);
                if (Directory.Exists(backupDir))
                {
                    Directory.Move(backupDir, appDir);
                }
                throw;
            }
        }

        private static void CopyDirectory(string source, string destination)
        {
            Directory.CreateDirectory(destination);
            foreach (var dir in Directory.GetDirectories(source, "*", SearchOption.AllDirectories))
            {
                Directory.CreateDirectory(Path.Combine(destination, RelativePath(source, dir)));
            }
            foreach (var file in Directory.GetFiles(source, "*", SearchOption.AllDirectories))
            {
                var target = Path.Combine(destination, RelativePath(source, file));
                Directory.CreateDirectory(Path.GetDirectoryName(target));
                File.Copy(file, target, true);
            }
        }

        private static string RelativePath(string root, string path)
        {
            var rootUri = new Uri(AppendDirectorySeparator(Path.GetFullPath(root)));
            var pathUri = new Uri(Path.GetFullPath(path));
            return Uri.UnescapeDataString(rootUri.MakeRelativeUri(pathUri).ToString()).Replace('/', Path.DirectorySeparatorChar);
        }

        private static string AppendDirectorySeparator(string path)
        {
            if (path.EndsWith(Path.DirectorySeparatorChar.ToString())) return path;
            return path + Path.DirectorySeparatorChar;
        }

        private static void WriteLaunchers(string installDir)
        {
            var appPath = Path.Combine(installDir, "App", AppExeName);
            WriteCmd(Path.Combine(installDir, "Launch LightNovel Reader.cmd"), appPath);

            var startMenu = Path.Combine(
                Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
                "Microsoft",
                "Windows",
                "Start Menu",
                "Programs",
                "LightNovel Reader");
            Directory.CreateDirectory(startMenu);
            WriteCmd(Path.Combine(startMenu, "LightNovel Reader.cmd"), appPath);

            var desktop = Environment.GetFolderPath(Environment.SpecialFolder.DesktopDirectory);
            if (!String.IsNullOrWhiteSpace(desktop) && Directory.Exists(desktop))
            {
                WriteCmd(Path.Combine(desktop, "LightNovel Reader.cmd"), appPath);
            }
        }

        private static void WriteCmd(string path, string appPath)
        {
            var lines = new[]
            {
                "@echo off",
                "start \"\" \"" + appPath + "\""
            };
            File.WriteAllLines(path, lines, Encoding.ASCII);
        }

        private static void TryDelete(string path)
        {
            try
            {
                if (Directory.Exists(path)) Directory.Delete(path, true);
                else if (File.Exists(path)) File.Delete(path);
            }
            catch
            {
                // Best effort cleanup. Installation rollback handles critical directories separately.
            }
        }

        private static void Log(string message)
        {
            Console.WriteLine(message);
        }
    }
}
