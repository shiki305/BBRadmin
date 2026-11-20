using System.Collections.Generic;
using System.Linq;
using System.Net.NetworkInformation;
using System.Net.Sockets;

namespace BBRadmin.Core
{
    /// <summary>One address this PC has, with the kind of network it belongs to.</summary>
    internal sealed class NetworkChoice
    {
        public string Adapter;
        public string Ip;
        public string Kind;

        public string Describe()
        {
            return Kind + " · " + Ip + " · " + Adapter;
        }

        public override string ToString()
        {
            return Describe();
        }
    }

    /// <summary>
    /// The addresses the friends can reach this PC on. Radmin VPN's 26.0.0.0/8 ranks first, then the
    /// other VPNs BBSC knows (Hamachi 25/8, Tailscale 100.64/10, ZeroTier), then a local address.
    /// </summary>
    internal static class Radmin
    {
        public static List<NetworkChoice> Detect()
        {
            List<NetworkChoice> found = new List<NetworkChoice>();
            foreach (NetworkInterface nic in NetworkInterface.GetAllNetworkInterfaces())
            {
                if (nic.OperationalStatus != OperationalStatus.Up) continue;
                if (nic.NetworkInterfaceType == NetworkInterfaceType.Loopback) continue;
                foreach (UnicastIPAddressInformation address in nic.GetIPProperties().UnicastAddresses)
                {
                    if (address.Address.AddressFamily != AddressFamily.InterNetwork) continue;
                    string ip = address.Address.ToString();
                    if (ip.StartsWith("169.254.")) continue;  // no DHCP answer
                    NetworkChoice choice = new NetworkChoice();
                    choice.Adapter = nic.Name;
                    choice.Ip = ip;
                    choice.Kind = KindOf(ip, nic.Name);
                    found.Add(choice);
                }
            }
            return found.OrderBy(c => Rank(c.Kind)).ThenBy(c => c.Ip).ToList();
        }

        /// <summary>The address a start uses: the first one a friend can reach (Radmin first).</summary>
        public static NetworkChoice Pick(List<NetworkChoice> all)
        {
            if (all == null || all.Count == 0) return null;
            return all[0];
        }

        public static bool IsVpn(string kind)
        {
            return kind != null && kind != "Rede local" && kind != "Outra rede";
        }

        private static int Rank(string kind)
        {
            if (kind == "Radmin VPN") return 0;
            if (kind == "Hamachi") return 1;
            if (kind == "Tailscale") return 2;
            if (kind == "ZeroTier") return 3;
            if (kind == "Rede local") return 4;
            return 5;
        }

        private static string KindOf(string ip, string adapter)
        {
            string[] parts = ip.Split('.');
            int first = 0, second = 0;
            if (parts.Length == 4)
            {
                int.TryParse(parts[0], out first);
                int.TryParse(parts[1], out second);
            }
            string both = (adapter + " " + ip).ToLowerInvariant();
            if (both.Contains("radmin") || first == 26) return "Radmin VPN";
            if (both.Contains("hamachi") || first == 25) return "Hamachi";
            if (both.Contains("tailscale") || (first == 100 && second >= 64 && second <= 127)) return "Tailscale";
            if (both.Contains("zerotier")) return "ZeroTier";
            if (first == 10 || first == 172 || first == 192) return "Rede local";
            return "Outra rede";
        }
    }
}
