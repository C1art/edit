use serde::{Deserialize, Serialize};

use crate::{AutoReframe, Param, Stabilizer};

mod catalog;
mod detail;
mod echo;
mod edges;
mod garbage;
mod generate;
mod key;
mod lighting;
mod names;
mod options;
mod params;
mod pattern;
mod shadow;
mod shift;
mod text;
mod tone;

pub use detail::{EdgeFeather, Median, NoiseHls, UnsharpMask};
pub use echo::{ChannelBlur, Echo, EchoOperator};
pub use edges::{AlphaGlow, Bevel, RoughenEdgeType, RoughenEdges};
pub use garbage::GarbageMatte;
pub use generate::{Checkerboard, Circle, FourColorGradient, Grid};
pub use key::{KeyOutput, MatteChannel, ULTRA_KEY_REACH};
pub use lighting::{Light, LightKind, LightingEffects, MAX_LIGHTS};
pub use options::{
    ClipNameSource, ColorChange, ColorMatch, EffectCategory, LensType, MagnifyShape,
    RadialWipeDirection, RampShape, Rgb, TimecodeSource,
};
pub use pattern::{CellKind, CellPattern, Ellipse, Lightning};
pub use shadow::{RadialShadow, ShadowRender};
pub use shift::{ChangeColor, ChangeColorView, ColorBalance, MatchColors};
pub use text::SimpleText;
pub use tone::{
    AscCdl, ColorReplace, ConvolutionKernel, Extract, LimiterMethod, ShadowHighlight, VideoLimiter,
};

/// A video effect from the effect stack. Percentages are stored as their numbers.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VideoEffect {
    GaussianBlur {
        /// Blur radius in sequence pixels.
        blurriness: Param,
    },
    HorizontalFlip,
    VerticalFlip,
    BlackAndWhite,
    /// Maps the picture's luminance onto a ramp from `black` to `white`.
    Tint {
        black: Rgb,
        white: Rgb,
        amount: Param,
    },
    Invert {
        amount: Param,
    },
    Sharpen {
        amount: Param,
    },
    /// Darkens (negative amounts) or lightens the picture's edges.
    Vignette {
        amount: Param,
        midpoint: Param,
        feather: Param,
    },
    /// Keys out colors near `color`, as a green or blue screen. The matte is then
    /// cleaned up: `pedestal` clears faint noise from the background, `contrast`
    /// hardens its partly transparent edge, `choke` shrinks it and `soften` blurs it,
    /// both in percent of `ULTRA_KEY_REACH`. `spill` takes the key's color out of what
    /// remains.
    UltraKey {
        color: Rgb,
        tolerance: Param,
        softness: Param,
        #[serde(default)]
        output: KeyOutput,
        #[serde(default)]
        pedestal: Param,
        #[serde(default)]
        contrast: Param,
        #[serde(default)]
        choke: Param,
        #[serde(default)]
        soften: Param,
        #[serde(default = "key::default_spill")]
        spill: Param,
    },
    /// Shows the clip only where the picture of another video track is, as Premiere's
    /// Track Matte Key does. The matte is that track's picture in sequence space, so
    /// moving the clip moves it behind the matte.
    TrackMatteKey {
        /// The track's index among the video tracks the clip sits with, bottom first.
        matte: Option<usize>,
        channel: MatteChannel,
        reverse: bool,
    },
    /// Track Matte Key with any channel of the matte track for the matte, as Premiere's
    /// Set Matte takes one from another layer.
    SetMatte {
        matte: Option<usize>,
        channel: MatteChannel,
        reverse: bool,
    },
    /// Both -100 to 100, on the encoded picture as a display shows it.
    BrightnessContrast {
        brightness: Param,
        contrast: Param,
    },
    /// An exponent on the encoded picture; above 1 darkens the midtones.
    GammaCorrection {
        gamma: Param,
    },
    /// Steps of brightness per channel, 2 to 64.
    Posterize {
        levels: Param,
    },
    /// Grain that changes every frame, in percent.
    Noise {
        amount: Param,
    },
    /// Outlines where the picture changes, dark lines on white unless inverted.
    FindEdges {
        invert: bool,
        /// Percent of the original showing through.
        blend: Param,
    },
    /// Blocks across and down the picture, each one color.
    Mosaic {
        horizontal: Param,
        vertical: Param,
    },
    /// Shifts the picture, wrapping it around, by percent of its width and height.
    Offset {
        shift: [Param; 2],
        /// Percent of the original showing through.
        blend: Param,
    },
    /// Turns the picture around its center, most at the center.
    Twirl {
        /// Degrees.
        angle: Param,
        /// Percent of half the picture's diagonal.
        radius: Param,
    },
    /// -100 (pinched) to 100 (bulging), as through a wide lens.
    LensDistortion {
        curvature: Param,
    },
    /// Smears the picture both ways along a direction.
    DirectionalBlur {
        /// Degrees clockwise from vertical.
        direction: Param,
        /// Sequence pixels.
        length: Param,
    },
    /// A shadow of the picture's shape behind it.
    DropShadow {
        color: Rgb,
        /// Percent.
        opacity: Param,
        /// Degrees clockwise from up that the shadow falls toward.
        direction: Param,
        /// Pixels of the clip's picture.
        distance: Param,
        softness: Param,
    },
    /// Ripples the picture with sine waves that travel as the clip plays.
    WaveWarp {
        /// Pixels the picture moves each way.
        height: Param,
        /// Pixels from one wave to the next.
        width: Param,
        /// Degrees clockwise from up that the waves travel toward.
        direction: Param,
        /// Waves passing a point each second.
        speed: Param,
    },
    /// Steadies a shaky camera from an analysis of the clip's motion.
    WarpStabilizer(Stabilizer),
    /// Keys out colors whose encoded values lie near `color`, with no spill handling.
    ColorKey {
        color: Rgb,
        /// Percent of the largest distance between colors.
        tolerance: Param,
        softness: Param,
    },
    /// Turns everything but colors near `color` grey.
    LeaveColor {
        color: Rgb,
        /// Percent the other colors lose.
        amount: Param,
        tolerance: Param,
        softness: Param,
        matching: ColorMatch,
    },
    /// Fills the picture with a blend from one color to another.
    GradientRamp {
        /// Percent of the picture's width and height.
        start: [Param; 2],
        start_color: Rgb,
        end: [Param; 2],
        end_color: Rgb,
        shape: RampShape,
        /// Percent of the original showing through.
        blend: Param,
    },
    /// Reflects the picture across a line through `center`, the side the line's
    /// normal points away from showing on both.
    Mirror {
        /// Percent of the picture's width and height.
        center: [Param; 2],
        /// Degrees clockwise; at 0 the left half shows on the right.
        angle: Param,
    },
    /// Moves the picture's corners, each in percent of its width and height: upper left,
    /// upper right, lower left, lower right.
    CornerPin {
        corners: [[Param; 2]; 4],
    },
    /// Holds each picture for a while, as footage shot at `frame_rate`.
    PosterizeTime {
        frame_rate: Param,
    },
    /// Flashes a color over the picture at regular intervals of the media's time.
    StrobeLight {
        color: Rgb,
        /// Percent of the color over the picture while it flashes.
        blend: Param,
        /// Seconds each flash lasts.
        duration: Param,
        /// Seconds from the start of one flash to the next.
        period: Param,
    },
    /// Burns the time into the picture, white on a box.
    Timecode {
        /// The box's center, in percent of the picture's width and height.
        position: [Param; 2],
        /// Text height in percent of the picture's height.
        size: Param,
        /// Percent opacity of the box behind the text.
        opacity: Param,
        source: TimecodeSource,
    },
    /// Burns the clip's name into the picture, white on a box, as the Timecode effect
    /// burns the time.
    ClipName {
        /// The box's center, in percent of the picture's width and height.
        position: [Param; 2],
        /// Text height in percent of the picture's height.
        size: Param,
        /// Percent opacity of the box behind the text.
        opacity: Param,
        source: ClipNameSource,
    },
    /// Moves, scales, skews and turns the picture about its anchor point, as Motion
    /// does but in its place among the effects.
    Transform {
        /// The point of the picture that turns and scales in place, in percent of its
        /// width and height.
        anchor: [Param; 2],
        /// Where the anchor point goes, in percent of the picture's width and height.
        position: [Param; 2],
        /// Percent of the width, then of the height.
        scale: [Param; 2],
        /// Degrees the picture slants by along the skew axis.
        skew: Param,
        /// Degrees clockwise from vertical.
        skew_axis: Param,
        /// Degrees clockwise.
        rotation: Param,
        /// Percent.
        opacity: Param,
    },
    /// Turns the picture in depth about its middle, as a card seen in perspective.
    Basic3D {
        /// Degrees about the vertical axis, the right side coming nearer.
        swivel: Param,
        /// Degrees about the horizontal axis, the bottom coming nearer.
        tilt: Param,
        /// How far the picture moves away, in percent of its height.
        distance: Param,
    },
    /// Bulges the picture inside a circle as if it were wrapped around a ball.
    Spherize {
        /// Pixels of the clip's picture.
        radius: Param,
        /// Percent of the picture's width and height.
        center: [Param; 2],
    },
    /// Shows the picture's edges in grey relief, lit from one side.
    Emboss {
        /// Degrees clockwise from up that the light comes from.
        direction: Param,
        /// Pixels of the clip's picture the relief stands out by.
        relief: Param,
        /// Percent.
        contrast: Param,
        /// Percent of the original showing through.
        blend: Param,
    },
    /// Pushes the picture about along a turbulent noise.
    TurbulentDisplace {
        /// How far, in percent of the noise's size.
        amount: Param,
        /// Pixels of the clip's picture across one swirl of the noise.
        size: Param,
        /// Where the noise is centered, in percent of the picture's width and height.
        offset: [Param; 2],
        /// Layers of finer noise, 1 to 10.
        complexity: Param,
        /// Degrees; turning it changes the noise smoothly.
        evolution: Param,
    },
    /// Remakes each channel from all three, in percent of each and a constant.
    ChannelMixer {
        /// Red from red, green, blue, then the constant.
        red: [Param; 4],
        green: [Param; 4],
        blue: [Param; 4],
        /// Every channel made as red is.
        monochrome: bool,
    },
    /// Moves the colors near one color's hue to another's.
    ChangeToColor {
        from: Rgb,
        to: Rgb,
        change: ColorChange,
        /// Percent of hue, lightness and saturation away from `from` that still match.
        tolerance: [Param; 3],
        /// Percent of the tolerance the match fades over.
        softness: Param,
    },
    /// Keys out the picture by its brightness: dark areas when `threshold` is above
    /// `cutoff`, bright ones when it is below. Between the two the picture fades.
    LumaKey {
        /// Percent of encoded luma from which the picture is opaque.
        threshold: Param,
        /// Percent of encoded luma at which it is fully transparent.
        cutoff: Param,
    },
    /// Changes how the picture's alpha channel is read.
    AlphaAdjust {
        /// Percent.
        opacity: Param,
        ignore_alpha: bool,
        invert_alpha: bool,
        /// Shows the alpha channel in grey levels instead of the picture.
        mask_only: bool,
    },
    /// Inverts each encoded channel brighter than `threshold`, in percent.
    Solarize {
        threshold: Param,
    },
    /// Turns the picture black where its encoded luma is below `level`, 0 to 255, and
    /// white elsewhere.
    Threshold {
        level: Param,
    },
    /// Tiles copies of the picture, `count` across and down.
    Replicate {
        count: Param,
    },
    /// Percent of each encoded channel, red, green then blue.
    ColorBalanceRgb {
        gains: [Param; 3],
    },
    Checkerboard(Checkerboard),
    Grid(Grid),
    Circle(Circle),
    FourColorGradient(FourColorGradient),
    /// Adds the glare and reflections of a bright light shining into the lens.
    LensFlare {
        /// Percent of the picture's width and height.
        center: [Param; 2],
        /// Percent.
        brightness: Param,
        lens: LensType,
        /// Percent of the original showing through.
        blend: Param,
    },
    /// Enlarges the picture within a circle or square about `center`.
    Magnify {
        shape: MagnifyShape,
        /// Percent of the picture's width and height.
        center: [Param; 2],
        /// Percent.
        magnification: Param,
        /// The shape's radius, or half its side, in the picture's pixels, as is the
        /// feather softening its edge.
        size: Param,
        feather: Param,
        /// Percent.
        opacity: Param,
    },
    /// The Transition effects hide `completion` percent of the picture, their feathers
    /// in the picture's pixels and their angles in degrees clockwise from up.
    LinearWipe {
        completion: Param,
        /// The way the wipe travels across the picture.
        angle: Param,
        feather: Param,
    },
    RadialWipe {
        completion: Param,
        start_angle: Param,
        /// Percent of the picture's width and height.
        center: [Param; 2],
        direction: RadialWipeDirection,
        feather: Param,
    },
    /// Stripes `width` pixels wide that each close across `direction`.
    VenetianBlinds {
        completion: Param,
        direction: Param,
        width: Param,
        feather: Param,
    },
    /// Maps the encoded levels from `input_black`..`input_white` onto
    /// `output_black`..`output_white`, all from 0 to 255, bending them by `gamma`.
    Levels {
        input_black: Param,
        input_white: Param,
        output_black: Param,
        output_white: Param,
        gamma: Param,
    },
    /// Adjusts the picture as a video processing amplifier does: brightness in 8-bit
    /// levels, contrast and saturation in percent and hue in degrees.
    ProcAmp {
        brightness: Param,
        contrast: Param,
        hue: Param,
        saturation: Param,
    },
    /// Blocks of the picture vanishing in a fixed random order.
    BlockDissolve {
        completion: Param,
        block_width: Param,
        block_height: Param,
        feather: Param,
    },
    UnsharpMask(UnsharpMask),
    Median(Median),
    DustAndScratches(Median),
    EdgeFeather(EdgeFeather),
    NoiseHls(NoiseHls),
    ChannelBlur(ChannelBlur),
    /// Blurs the picture as an out-of-focus lens does, by half a pixel of blurriness
    /// per percent.
    CameraBlur {
        percent: Param,
    },
    Echo(Echo),
    GarbageMatte(GarbageMatte),
    /// Fills a frame of another shape with the picture, following its subject.
    AutoReframe(AutoReframe),
    RoughenEdges(RoughenEdges),
    BevelEdges(Bevel),
    BevelAlpha(Bevel),
    AlphaGlow(AlphaGlow),
    /// Emboss's relief, lighting the picture's colors rather than grey.
    ColorEmboss {
        direction: Param,
        relief: Param,
        contrast: Param,
        blend: Param,
    },
    ConvolutionKernel(ConvolutionKernel),
    Extract(Extract),
    ShadowHighlight(ShadowHighlight),
    VideoLimiter(VideoLimiter),
    AscCdl(AscCdl),
    ColorReplace(ColorReplace),
    ChangeColor(ChangeColor),
    ColorBalance(ColorBalance),
    SimpleText(SimpleText),
    Ellipse(Ellipse),
    CellPattern(CellPattern),
    Lightning(Lightning),
    LightingEffects(LightingEffects),
    RadialShadow(RadialShadow),
}
