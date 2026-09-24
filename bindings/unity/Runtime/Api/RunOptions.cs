// Xybrid SDK - Per-run abort policy and cloud configuration.
using System;

namespace Xybrid
{
    /// <summary>Device-pressure signals which can stop a run.</summary>
    public enum AbortSignal
    {
        MemoryPressureWarn,
        MemoryPressureCritical,
        ThermalHot,
        ThermalCritical
    }

    /// <summary>Per-run controls; sampling settings remain in GenerationConfig.</summary>
    /// <remarks>
    /// Cloud defaults belong to the app. Null or blank overrides leave existing
    /// envelope metadata alone. Supplying overrides does not enable fallback;
    /// FallbackToCloud must be true. Use matching generated bindings and natives.
    /// </remarks>
    public sealed class RunOptions
    {
        public AbortSignal[] AbortOn { get; set; } = Array.Empty<AbortSignal>();
        public bool FallbackToCloud { get; set; }
        public uint MaxGraceTokens { get; set; }
        public string CorrelationId { get; set; }
        public string CloudProvider { get; set; }
        public string CloudModel { get; set; }
        /// <summary>Gateway /v1 base; validated by the facade when fallback is enabled.</summary>
        public string CloudGatewayUrl { get; set; }

        internal XybridBolt.XybridRunOptions ToBolt(GenerationConfig config)
        {
            var signals = Array.ConvertAll(AbortOn ?? Array.Empty<AbortSignal>(), signal => signal switch
            {
                AbortSignal.MemoryPressureWarn => XybridBolt.XybridAbortSignal.MemoryPressureWarn,
                AbortSignal.MemoryPressureCritical => XybridBolt.XybridAbortSignal.MemoryPressureCritical,
                AbortSignal.ThermalHot => XybridBolt.XybridAbortSignal.ThermalHot,
                AbortSignal.ThermalCritical => XybridBolt.XybridAbortSignal.ThermalCritical,
                _ => throw new ArgumentOutOfRangeException(nameof(AbortOn))
            });
            return new XybridBolt.XybridRunOptions(config?.ToBolt(), signals,
                FallbackToCloud, MaxGraceTokens, CorrelationId,
                CloudProvider, CloudModel, CloudGatewayUrl);
        }
    }
}
