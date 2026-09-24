using System;
using System.Runtime.InteropServices;
using NUnit.Framework;
using XybridBolt;

namespace Xybrid.Tests.Editor
{
    [TestFixture]
    public class RunOptionsTests
    {
        [Test]
        public void OldConstructorAndWrapperDefaultsDoNotOptIntoCloud()
        {
            var legacy = new XybridRunOptions(null, Array.Empty<XybridAbortSignal>(), false, 0u, null);
            Assert.IsNull(legacy.CloudProvider);
            Assert.IsNull(legacy.CloudModel);
            Assert.IsNull(legacy.CloudGatewayUrl);
            var options = new RunOptions().ToBolt(null);
            Assert.IsFalse(options.FallbackToCloud);
            Assert.IsNull(options.GenerationConfig);
            Assert.IsEmpty(options.AbortOn);
        }

        [Test]
        public void WrapperForwardsOverridesWithoutEnablingFallback()
        {
            var options = new RunOptions {
                CloudProvider = "provider-x", CloudModel = "model-y",
                CloudGatewayUrl = "https://api.xybrid.dev/v1", CorrelationId = "trace",
                MaxGraceTokens = 7, AbortOn = new[] { AbortSignal.ThermalCritical }
            }.ToBolt(null);
            Assert.IsFalse(options.FallbackToCloud);
            Assert.AreEqual("provider-x", options.CloudProvider);
            Assert.AreEqual("model-y", options.CloudModel);
            Assert.AreEqual("https://api.xybrid.dev/v1", options.CloudGatewayUrl);
            Assert.AreEqual("trace", options.CorrelationId);
            Assert.AreEqual(7u, options.MaxGraceTokens);
            Assert.AreEqual(new[] { XybridAbortSignal.ThermalCritical }, options.AbortOn);
        }

        [Test]
        public void NewFieldsRoundTripAtTheWireTail()
        {
            var options = new RunOptions { FallbackToCloud = true, CloudProvider = "provider-x",
                CloudModel = "model-y", CloudGatewayUrl = "https://api.xybrid.dev/v1" }.ToBolt(null);
            var writer = new WireWriter();
            options.Encode(writer);
            var bytes = writer.ToArray();
            var handle = GCHandle.Alloc(bytes, GCHandleType.Pinned);
            try {
                var decoded = XybridRunOptions.Decode(new WireReader(handle.AddrOfPinnedObject(), checked((nuint)bytes.Length)));
                Assert.IsTrue(decoded.FallbackToCloud);
                Assert.AreEqual(options.CloudProvider, decoded.CloudProvider);
                Assert.AreEqual(options.CloudModel, decoded.CloudModel);
                Assert.AreEqual(options.CloudGatewayUrl, decoded.CloudGatewayUrl);
            } finally { handle.Free(); }
        }

        [Test]
        public void UnknownAbortSignalIsNotSilentlyDropped()
        {
            var options = new RunOptions { AbortOn = new[] { (AbortSignal)999 } };
            Assert.Throws<ArgumentOutOfRangeException>(() => options.ToBolt(null));
        }

        // Compile every old call shape and the new named-options form. Native
        // execution itself is covered by SDK integration tests, not this fixture.
        private static void CallShapes(Model model, Envelope envelope, ConversationContext context)
        {
            model.Run(envelope);
            model.Run(envelope, context);
            model.RunStreaming(envelope, _ => { });
            model.RunStreaming(envelope, context, _ => { });
            var options = new RunOptions { FallbackToCloud = true, CloudModel = "model-y" };
            model.Run(envelope, options: options);
            model.Run(envelope, context, options: options);
            model.RunStreaming(envelope, _ => { }, options: options);
            model.RunStreaming(envelope, context, _ => { }, options: options);
        }
    }
}
