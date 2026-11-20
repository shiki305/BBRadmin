using System;
using System.Globalization;
using System.Net;
using System.Net.Sockets;
using System.Text;

namespace BBRadmin.Core
{
    /// <summary>The three questions a co-op start asks of a server: the link, the WebAPI and the backend.</summary>
    internal static class Health
    {
        public static bool Tcp(string host, int port, int timeoutMs)
        {
            TcpClient client = new TcpClient();
            try
            {
                IAsyncResult pending = client.BeginConnect(host, port, null, null);
                if (!pending.AsyncWaitHandle.WaitOne(timeoutMs, false)) return false;
                client.EndConnect(pending);
                return true;
            }
            catch (Exception)
            {
                return false;
            }
            finally
            {
                try { client.Close(); }
                catch (Exception) { }
            }
        }

        /// <summary>The WebAPI's answer (it serves /status), or "" when nothing answers.</summary>
        public static string HttpStatus(string url, int timeoutMs)
        {
            try
            {
                HttpWebRequest request = (HttpWebRequest)WebRequest.Create(url);
                request.Timeout = timeoutMs;
                request.ReadWriteTimeout = timeoutMs;
                request.Method = "GET";
                request.Proxy = null;
                using (HttpWebResponse response = (HttpWebResponse)request.GetResponse())
                {
                    return ((int)response.StatusCode).ToString(CultureInfo.InvariantCulture) + " " + response.StatusDescription;
                }
            }
            catch (WebException error)
            {
                if (error.Response != null)
                {
                    using (HttpWebResponse response = (HttpWebResponse)error.Response)
                    {
                        // any answer at all means the port is served
                        return ((int)response.StatusCode).ToString(CultureInfo.InvariantCulture) + " " + response.StatusDescription;
                    }
                }
                return "";
            }
            catch (Exception)
            {
                return "";
            }
        }

        /// <summary>
        /// True when a server answers at this address: its WebAPI (/status) replies, or its backend port
        /// accepts. Both travel over TCP, so they can be asked from here. The game's link (31314) is UDP:
        /// there is no handshake to probe, which is why "the link is silent" is never a failure on its own.
        /// </summary>
        public static bool Reachable(string hostIp, int timeoutMs)
        {
            if (HttpStatus("http://" + hostIp + ":" + CoopSchema.WebApiPort.ToString(CultureInfo.InvariantCulture) + "/status", timeoutMs).Length > 0)
            {
                return true;
            }
            return Tcp(hostIp, CoopSchema.BackendPort, timeoutMs);
        }

        /// <summary>What to say about a server at this address: one line, no colours.</summary>
        public static string Describe(string hostIp, int timeoutMs)
        {
            StringBuilder text = new StringBuilder();
            text.Append("WebAPI ").Append(CoopSchema.WebApiPort.ToString(CultureInfo.InvariantCulture)).Append(": ");
            string status = HttpStatus("http://" + hostIp + ":" + CoopSchema.WebApiPort.ToString(CultureInfo.InvariantCulture) + "/status", timeoutMs);
            text.Append(status.Length > 0 ? "responde (" + status + ")" : "sem resposta");
            text.Append(" · backend ").Append(CoopSchema.BackendPort.ToString(CultureInfo.InvariantCulture))
                .Append(": ").Append(Tcp(hostIp, CoopSchema.BackendPort, timeoutMs) ? "responde" : "sem resposta");
            text.Append(" · link UDP ").Append(CoopSchema.LinkPort.ToString(CultureInfo.InvariantCulture));
            return text.ToString();
        }
    }
}
