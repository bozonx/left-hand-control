// Test-only observer. Capture raw physical scan codes before the mapper's hook;
// the callback queues records and never changes or blocks the keyboard stream.
using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Threading;
using System.Windows.Forms;

public static class LhcKeyTrace
{
    [DllImport("user32.dll")]
    public static extern IntPtr GetForegroundWindow();

    [StructLayout(LayoutKind.Sequential)]
    private struct KeyData
    {
        public uint vk, scan, flags, time;
        public UIntPtr extra;
    }
    private delegate IntPtr HookProc(int code, IntPtr message, IntPtr data);
    private static readonly HookProc Callback = Observe;
    private static readonly ConcurrentQueue<string> Records = new ConcurrentQueue<string>();
    private static readonly ManualResetEvent Ready = new ManualResetEvent(false);
    private static Thread worker;
    private static IntPtr hook;
    private static uint threadId;
    private static int error;

    [DllImport("user32.dll", SetLastError = true)]
    private static extern IntPtr SetWindowsHookExW(int kind, HookProc callback, IntPtr module, uint thread);
    [DllImport("user32.dll")]
    private static extern IntPtr CallNextHookEx(IntPtr hook, int code, IntPtr message, IntPtr data);
    [DllImport("user32.dll")]
    private static extern bool UnhookWindowsHookEx(IntPtr hook);
    [DllImport("user32.dll")]
    private static extern bool PostThreadMessageW(uint thread, uint message, IntPtr wparam, IntPtr lparam);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)]
    private static extern IntPtr GetModuleHandleW(string name);
    [DllImport("kernel32.dll")]
    private static extern uint GetCurrentThreadId();

    [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern IntPtr LoadKeyboardLayoutW(string identifier, uint flags);

    [DllImport("user32.dll")]
    private static extern short GetAsyncKeyState(int key);

    public static bool IsKeyDown(int key) { return GetAsyncKeyState(key) < 0; }

    private static IntPtr Observe(int code, IntPtr message, IntPtr pointer)
    {
        if (code == 0)
        {
            KeyData key = (KeyData)Marshal.PtrToStructure(pointer, typeof(KeyData));
            if ((key.flags & 0x10) == 0)
                Records.Enqueue("{\"vk\":" + key.vk + ",\"scan\":" + key.scan +
                    ",\"flags\":" + key.flags + ",\"time\":" + key.time + "}");
        }
        return CallNextHookEx(IntPtr.Zero, code, message, pointer);
    }

    public static void Start()
    {
        if (worker != null) return;
        Ready.Reset();
        worker = new Thread(delegate()
        {
            threadId = GetCurrentThreadId();
            hook = SetWindowsHookExW(13, Callback, GetModuleHandleW(null), 0);
            error = Marshal.GetLastWin32Error();
            Ready.Set();
            if (hook == IntPtr.Zero) return;
            try { Application.Run(); }
            finally { UnhookWindowsHookEx(hook); }
        });
        worker.IsBackground = true;
        worker.SetApartmentState(ApartmentState.STA);
        worker.Start();
        if (!Ready.WaitOne(5000)) throw new Exception("Keyboard trace did not start");
        if (hook == IntPtr.Zero) throw new Win32Exception(error);
    }

    public static string[] Drain()
    {
        List<string> records = new List<string>();
        string record;
        while (Records.TryDequeue(out record)) records.Add(record);
        return records.ToArray();
    }

    public static void Stop()
    {
        if (worker == null) return;
        if (worker.IsAlive)
        {
            PostThreadMessageW(threadId, 0x12, IntPtr.Zero, IntPtr.Zero);
            if (!worker.Join(5000)) throw new Exception("Keyboard trace did not stop");
        }
        worker = null;
        hook = IntPtr.Zero;
    }
}
