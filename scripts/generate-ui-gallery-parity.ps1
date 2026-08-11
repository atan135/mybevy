[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repositoryRoot = Split-Path -Parent $PSScriptRoot
$outputPath = Join-Path $repositoryRoot "project/assets/ui/documents/approved/gallery/declarative_gallery.v1.json"

$galleryI18nTargets = @{
    "gallery.title|content" = "ui_document_gallery.title"
    "gallery.nav.lobby|label" = "nav.lobby"
    "gallery.section.visual_foundation.title|content" = "ui_gallery.visual_foundation.section"
    "gallery.section.visual_foundation.description|content" = "ui_gallery.visual_foundation.description"
    "gallery.section.image_fit.title|content" = "ui_gallery.image_fit.section"
    "gallery.section.image_fit.description|content" = "ui_gallery.image_fit.description"
    "gallery.section.visual_acceptance.title|content" = "ui_gallery.visual_acceptance.section"
    "gallery.section.visual_acceptance.description|content" = "ui_gallery.visual_acceptance.description"
    "gallery.pair.visual_acceptance.shadow_gradient.label|content" = "ui_gallery.effects.composite"
    "gallery.pair.visual_acceptance.selected|label" = "ui_gallery.buttons.selected"
    "gallery.pair.visual_acceptance.loading|label" = "ui_gallery.buttons.loading"
    "gallery.pair.visual_acceptance.disabled|label" = "ui_gallery.buttons.disabled"
    "gallery.section.image_modes.title|content" = "ui_gallery.image_modes.section"
    "gallery.section.image_modes.description|content" = "ui_gallery.image_modes.description"
    "gallery.section.typography.title|content" = "ui_gallery.typography.section"
    "gallery.section.typography.description|content" = "ui_gallery.typography.description"
    "gallery.typography.weights|content" = "ui_gallery.typography.weights"
    "gallery.section.typography_overflow.title|content" = "ui_gallery.typography.overflow"
    "gallery.section.typography_boundary.title|content" = "ui_gallery.typography.boundary"
    "gallery.section.buttons.title|content" = "ui_gallery.buttons.section"
    "gallery.section.icons.title|content" = "ui_gallery.icon_buttons.section"
    "gallery.section.icons.description|content" = "ui_gallery.icon_buttons.description"
    "gallery.icons.icon_only|content" = "ui_gallery.icon_buttons.icon_only"
    "gallery.icons.labeled|content" = "ui_gallery.icon_buttons.labeled"
    "gallery.section.icon_states.title|content" = "ui_gallery.icon_states.section"
    "gallery.section.icon_states.description|content" = "ui_gallery.icon_states.description"
    "gallery.section.style_scopes.title|content" = "ui_gallery.style_scopes.section"
    "gallery.section.style_scopes.description|content" = "ui_gallery.style_scopes.description"
    "gallery.section.effects.title|content" = "ui_gallery.effects.section"
    "gallery.section.effects.description|content" = "ui_gallery.effects.description"
    "gallery.section.animations.title|content" = "ui_gallery.animations.section"
    "gallery.section.animations.description|content" = "ui_gallery.animations.description"
    "gallery.section.components.title|content" = "ui_gallery.components.section"
    "gallery.section.components.description|content" = "ui_gallery.components.description"
    "gallery.components.badge|content" = "ui_gallery.components.badge"
    "gallery.components.progress|content" = "ui_gallery.components.progress"
    "gallery.components.tabs|content" = "ui_gallery.components.tabs"
    "gallery.components.dropdown|content" = "ui_gallery.components.dropdown"
    "gallery.select|placeholder" = "ui_gallery.components.dropdown.normal"
    "gallery.components.tooltip|content" = "ui_gallery.components.tooltip"
    "gallery.tooltip|body" = "ui_gallery.components.tooltip.body"
    "gallery.tooltip.target|content" = "ui_gallery.components.tooltip.normal"
    "gallery.pair.components.tooltip.error|body" = "ui_gallery.components.tooltip.error_body"
    "gallery.tooltip.error.target|content" = "ui_gallery.components.tooltip.error"
    "gallery.pair.components.tooltip.disabled|body" = "ui_gallery.components.tooltip.disabled_body"
    "gallery.tooltip.disabled.target|content" = "ui_gallery.components.tooltip.disabled"
    "gallery.components.selection_states|content" = "ui_gallery.components.selection_states"
    "gallery.section.selection.title|content" = "ui_gallery.selection.section"
    "gallery.section.numeric.title|content" = "ui_gallery.numeric.section"
    "gallery.pair.numeric.volume|label" = "ui_gallery.numeric.slider.volume"
    "gallery.pair.numeric.slider_disabled|label" = "ui_gallery.numeric.slider.disabled"
    "gallery.pair.numeric.players|label" = "ui_gallery.numeric.stepper.players"
    "gallery.pair.numeric.stepper_disabled|label" = "ui_gallery.numeric.stepper.disabled"
    "gallery.section.inputs.title|content" = "ui_gallery.inputs.section"
    "gallery.section.binding.title|content" = "ui_gallery.binding.section"
    "gallery.section.binding.description|content" = "ui_gallery.binding.description"
    "gallery.pair.binding.notice|content" = "ui_gallery.binding.notice"
    "gallery.pair.binding.bound_button|label" = "ui_gallery.binding.bound_button"
    "gallery.pair.binding.update|label" = "ui_gallery.binding.action"
    "gallery.section.overlays.title|content" = "ui_gallery.overlays.section"
    "gallery.section.images.title|content" = "ui_gallery.images.section"
    "gallery.section.images.description|content" = "ui_gallery.images.description"
    "gallery.section.atlas_sources.title|content" = "ui_gallery.images.atlas_sources"
    "gallery.section.atlas_sources.description|content" = "ui_gallery.images.atlas_sources.description"
    "gallery.section.stress.title|content" = "ui_gallery.stress.section"
    "gallery.section.stress.description|content" = "ui_gallery.stress.description"
}

function Resolve-GalleryI18nKey([string]$NodeId, [string]$Field, [string]$Fallback) {
    $target = "$NodeId|$Field"
    if ($galleryI18nTargets.ContainsKey($target)) {
        return $galleryI18nTargets[$target]
    }

    switch -Regex ($target) {
        '^gallery\.pair\.typography\.(large_title|section_title|subtitle|body|caption|button)\|content$' {
            return "ui_gallery.typography.$($Matches[1])"
        }
        '^gallery\.pair\.button\.(primary|secondary|focused|selected|loading|disabled|unavailable|action)\|label$' {
            return "ui_gallery.buttons.$($Matches[1])"
        }
        '^gallery\.pair\.icon\.(add|remove|help|close|loading|previous|next|tintable|full_color|missing)\|label$' {
            return "ui_gallery.icon_buttons.$($Matches[1])"
        }
        '^gallery\.pair\.icon_state\.(idle|hovered|pressed|focused|selected|disabled|loading)\|label$' {
            return "ui_gallery.icon_states.$($Matches[1])"
        }
        '^gallery\.pair\.style\.(global|parent|nested|restored|selected_button|selected_icon)(?:\.label)?\|(content|label)$' {
            return "ui_gallery.style_scopes.$($Matches[1])"
        }
        '^gallery\.pair\.effect\.(box_shadow|text_shadow|gradient|composite)\.label\|content$' {
            return "ui_gallery.effects.$($Matches[1])"
        }
        '^gallery\.pair\.effect\.material\.label\|content$' {
            return "ui_gallery.effects.material_fallback"
        }
        '^gallery\.pair\.animation\.control\|label$' {
            return "ui_gallery.animations.control"
        }
        '^gallery\.pair\.animation\.(page_entry|dialog_entry|dialog_exit|loading_loop|layout_size|color_transition|alpha_transition)\.label\|content$' {
            $animationKeys = @{
                page_entry = "page_entry"; dialog_entry = "modal_entry"; dialog_exit = "modal_exit"
                loading_loop = "loading"; layout_size = "layout"; color_transition = "color"; alpha_transition = "alpha"
            }
            return "ui_gallery.animations.$($animationKeys[$Matches[1]])"
        }
        '^gallery\.pair\.components\.badge\.(normal|selected|disabled|loading|empty|error)\|label$' {
            return "ui_gallery.components.state.$($Matches[1])"
        }
        '^gallery\.pair\.components\.progress\.(normal|disabled|loading|empty|error)\|label$' {
            return "ui_gallery.components.progress.$($Matches[1])"
        }
        '^gallery\.pair\.components\.tab\.(normal|hovered|pressed|focused|selected|disabled|loading)\|label$' {
            return "ui_gallery.components.tab.$($Matches[1])"
        }
        '^gallery\.pair\.components\.dropdown\.(hovered|pressed|focused|selected|disabled|loading|empty|error)\|placeholder$' {
            return "ui_gallery.components.dropdown.$($Matches[1])"
        }
        '^gallery\.(?:select|pair\.components\.dropdown\.[^.]+)\|option:(north|locked|long|south)$' {
            return "ui_gallery.components.dropdown.option.$($Matches[1])"
        }
        '^gallery\.pair\.components\.(checkboxes|toggles|segmented)\.(normal|hovered|pressed|focused|selected|disabled|loading|error)\|(label|option:[^|]+)$' {
            $family = @{ checkboxes = "checkbox"; toggles = "toggle"; segmented = "segment" }[$Matches[1]]
            return "ui_gallery.components.$family.$($Matches[2])"
        }
        '^gallery\.pair\.selection\.checkbox\.(unchecked|checked|disabled)\|label$' {
            return "ui_gallery.selection.checkbox.$($Matches[1])"
        }
        '^gallery\.pair\.selection\.toggle\.(off|on|disabled)\|label$' {
            return "ui_gallery.selection.toggle.$($Matches[1])"
        }
        '^gallery\.pair\.selection\.segmented\|option:(small|medium|large)$' {
            return "ui_gallery.selection.segment.$($Matches[1])"
        }
        '^gallery\.pair\.input\.(player_name|required|error|note|readonly|disabled|short_code|empty)\|(label|placeholder)$' {
            return "ui_gallery.inputs.placeholder.$($Matches[1])"
        }
        '^gallery\.pair\.input\.(player_name|required|note|readonly|short_code|empty)\|helper$' {
            return "ui_gallery.inputs.helper.$($Matches[1])"
        }
        '^gallery\.pair\.input\.required\|error$' {
            return "ui_gallery.inputs.validation.required"
        }
        '^gallery\.pair\.input\.error\|error$' {
            return "ui_gallery.inputs.validation.error"
        }
        '^gallery\.pair\.input\.disabled\|error$' {
            return "ui_gallery.inputs.validation.disabled_error"
        }
        '^gallery\.pair\.overlay\.(toast|loading|cancelable|hide|confirm|floating|close_top)\|label$' {
            $overlayKeys = @{ toast = "show_toast"; loading = "loading"; cancelable = "cancelable"; hide = "hide"; confirm = "show_confirm"; floating = "show_floating"; close_top = "close_top" }
            return "ui_gallery.overlays.$($overlayKeys[$Matches[1]])"
        }
        '^gallery\.pair\.stress\.item_(\d{2})\.title\|content$' {
            return "ui_gallery.stress.item_$($Matches[1])"
        }
        '^gallery\.pair\.stress\.item_\d{2}\.state\|content$' {
            $stateKeys = @{ Ready = "ready"; Waiting = "waiting"; Done = "done" }
            return "ui_gallery.stress.state.$($stateKeys[$Fallback])"
        }
        '^gallery\.pair\.stress\.item_\d{2}\.inspect\|label$' {
            return "ui_gallery.stress.action"
        }
    }
    return $null
}

function Set-GalleryI18nContent([object]$Content, [string]$NodeId, [string]$Field) {
    if ($null -eq $Content -or -not $Content.Contains("literal")) {
        return
    }
    $fallback = [string]$Content.literal
    $key = Resolve-GalleryI18nKey $NodeId $Field $fallback
    if ([string]::IsNullOrWhiteSpace($key)) {
        return
    }
    $Content.Remove("literal")
    $Content.i18n_key = $key
    $Content.fallback = $fallback
}

function Set-GalleryNodeI18n([object]$Node) {
    $nodeId = [string]$Node.id
    if ($Node.Contains("content")) {
        Set-GalleryI18nContent $Node.content $nodeId "content"
    }
    if ($Node.Contains("component") -and $Node.component.Contains("slots")) {
        foreach ($slot in $Node.component.slots.GetEnumerator()) {
            if ($slot.Value.Contains("content")) {
                Set-GalleryI18nContent $slot.Value.content $nodeId ([string]$slot.Key)
            }
        }
    }
    if ($Node.Contains("options")) {
        foreach ($option in $Node.options) {
            Set-GalleryI18nContent $option.label $nodeId "option:$($option.value)"
        }
    }
    if ($Node.Contains("children")) {
        foreach ($child in $Node.children) { Set-GalleryNodeI18n $child }
    }
    if ($Node.Contains("component") -and $Node.component.Contains("children")) {
        foreach ($child in $Node.component.children) { Set-GalleryNodeI18n $child }
    }
}

function New-TextContent([string]$Value) {
    return [ordered]@{ literal = $Value }
}

function New-TextNode(
    [string]$Id,
    [string]$Value,
    [string]$Role = "body",
    [string]$ColorRole = "primary"
) {
    $node = [ordered]@{
        type = "text"
        id = $Id
        content = New-TextContent $Value
        style = [ordered]@{ text_role = $Role }
    }
    if ($ColorRole -ne "primary") { $node.style.role = $ColorRole }
    return $node
}

function New-ContainerNode(
    [string]$Id,
    [object[]]$Children,
    [string]$Direction = "column",
    [bool]$Wrap = $false,
    [bool]$Panel = $false
) {
    $layout = [ordered]@{
        direction = $Direction
        gap = [ordered]@{ px = 12 }
    }
    if ($Wrap) {
        $layout.wrap = "wrap"
    }
    $node = [ordered]@{
        type = "container"
        id = $Id
        layout = $layout
        children = @($Children)
    }
    if ($Panel) {
        $node.style = [ordered]@{ component = "gallery_panel" }
        $layout.width = [ordered]@{ percent = 100 }
        $layout.max_width = [ordered]@{ px = 760 }
        $layout.align_self = "center"
        $layout.padding = [ordered]@{ all = [ordered]@{ px = 20 } }
    }
    return $node
}

function New-GridNode(
    [string]$Id,
    [object[]]$Children,
    [int]$Columns,
    [double]$Gap = 12
) {
    return [ordered]@{
        type = "container"
        id = $Id
        layout = [ordered]@{
            display = "grid"
            width = [ordered]@{ percent = 100 }
            grid_columns = @([ordered]@{ repeat = $Columns; size = [ordered]@{ fr = 1.0 } })
            grid_auto_rows = @("auto")
            row_gap = [ordered]@{ px = $Gap }
            column_gap = [ordered]@{ px = $Gap }
            align_items = "center"
            justify_items = "stretch"
        }
        children = @($Children)
    }
}

function New-GalleryCardNode(
    [string]$Id,
    [object[]]$Children
) {
    $node = New-ContainerNode $Id $Children
    $node.layout.width = [ordered]@{ percent = 100 }
    $node.layout.gap = [ordered]@{ px = 3 }
    $node.layout.padding = [ordered]@{ all = [ordered]@{ px = 6 } }
    $node.layout.overflow = [ordered]@{ x = "clip"; y = "clip" }
    $node.style = [ordered]@{ component = "gallery_image_card" }
    return $node
}

function New-ImageFrame(
    [string]$Id,
    [object]$Image,
    [object]$Width,
    [object]$Height,
    [double]$AspectRatio = 0
) {
    $layout = [ordered]@{
        width = $Width
        height = $Height
        overflow = [ordered]@{ x = "clip"; y = "clip" }
    }
    if ($AspectRatio -gt 0) { $layout.aspect_ratio = $AspectRatio }
    return [ordered]@{
        type = "container"
        id = $Id
        layout = $layout
        style = [ordered]@{ component = "gallery_image_frame" }
        children = @($Image)
    }
}

function New-ImageFitCard(
    [string]$Id,
    [string]$Label,
    [object]$Landscape,
    [object]$Portrait
) {
    $previews = New-ContainerNode "$Id.previews" @($Landscape, $Portrait) "row" $false
    $previews.layout.width = [ordered]@{ percent = 100 }
    $previews.layout.align_items = "center"
    $previews.layout.justify_content = "center"
    $previews.layout.gap = [ordered]@{ px = 6 }
    return [ordered]@{
        type = "container"
        id = $Id
        layout = [ordered]@{
            width = [ordered]@{ percent = 100 }
            direction = "column"
            gap = [ordered]@{ px = 3 }
            padding = [ordered]@{ all = [ordered]@{ px = 6 } }
            overflow = [ordered]@{ x = "clip"; y = "clip" }
        }
        style = [ordered]@{ component = "gallery_image_card" }
        children = @(
            $previews
            New-TextNode "$Id.label" $Label "caption" "muted"
        )
    }
}

function New-IconButtonNode(
    [string]$Id,
    [string]$Asset,
    [string]$State = "normal",
    [string]$Variant = "secondary",
    [string]$Label = "",
    [string]$AccessibleLabel = ""
) {
    $layout = [ordered]@{
        height = [ordered]@{ px = 46 }
        align_items = "center"
        justify_content = "center"
    }
    if ($Label) {
        $layout.min_width = [ordered]@{ px = 112 }
        $layout.direction = "row"
        $layout.column_gap = [ordered]@{ px = 6 }
    }
    else {
        $layout.width = [ordered]@{ px = 46 }
    }
    $controlLabel = if ($AccessibleLabel) { $AccessibleLabel } else { $Label }
    $node = [ordered]@{
        type = "image_button"
        id = $Id
        asset = $Asset
        layout = $layout
        component = New-Component $controlLabel $State $Variant
    }
    if (-not $Label) { $node.style = [ordered]@{ role = "icon_only" } }
    return $node
}

function New-IconSample(
    [string]$Id,
    [object]$Button,
    [string]$Label
) {
    return [ordered]@{
        type = "container"
        id = "$Id.sample"
        layout = [ordered]@{
            min_width = [ordered]@{ px = 76 }
            direction = "column"
            align_items = "center"
            gap = [ordered]@{ px = 6 }
        }
        children = @($Button, (New-TextNode "$Id.label" $Label "caption" "muted"))
    }
}

function New-SectionNode(
    [string]$Id,
    [string]$Title,
    [string]$Description,
    [object[]]$Children
) {
    $sectionChildren = @(
        New-TextNode "$Id.title" $Title "section_label" "muted"
    )
    if ($Description) {
        $sectionChildren += New-TextNode "$Id.description" $Description "body" "muted"
    }
    $sectionChildren += $Children
    return New-ContainerNode $Id $sectionChildren "column" $false $true
}

function New-Slot([string]$Value) {
    return [ordered]@{
        kind = "text"
        content = New-TextContent $Value
    }
}

function New-Component(
    [string]$Label,
    [string]$State = "normal",
    [string]$Variant = "default",
    [string]$Helper = "",
    [string]$Placeholder = "",
    [string]$ErrorText = ""
) {
    $slots = [ordered]@{}
    if ($Label) { $slots.label = New-Slot $Label }
    if ($Helper) { $slots.helper = New-Slot $Helper }
    if ($Placeholder) { $slots.placeholder = New-Slot $Placeholder }
    if ($ErrorText) { $slots.error = New-Slot $ErrorText }
    $component = [ordered]@{ slots = $slots }
    if ($Variant -ne "default") { $component.variant = $Variant }
    if ($State -ne "normal") { $component.states = @($State) }
    return $component
}

function New-ControlNode(
    [string]$Type,
    [string]$Id,
    [string]$Label,
    [string]$State = "normal",
    [string]$Variant = "default"
) {
    return [ordered]@{
        type = $Type
        id = $Id
        component = New-Component $Label $State $Variant
    }
}

function New-ImageNode(
    [string]$Id,
    [string]$Asset,
    [object]$Presentation,
    [double]$Width,
    [double]$Height
) {
    return [ordered]@{
        type = "image"
        id = $Id
        asset = $Asset
        presentation = $Presentation
        layout = [ordered]@{
            width = [ordered]@{ px = $Width }
            height = [ordered]@{ px = $Height }
        }
    }
}

function New-ImageCard(
    [string]$Id,
    [string]$Asset,
    [object]$Presentation,
    [string]$Label,
    [double]$Width,
    [double]$Height
) {
    return New-GalleryCardNode "$Id.card" @(
        New-ImageNode $Id $Asset $Presentation $Width $Height
        New-TextNode "$Id.label" $Label "caption" "muted"
    )
}

function New-FitPresentation([string]$Fit, [double]$X = 0.5, [double]$Y = 0.5) {
    return [ordered]@{
        kind = "fit"
        fit = $Fit
        focus = [ordered]@{ x = $X; y = $Y }
    }
}

function New-PackagedAsset(
    [string]$Kind,
    [string]$Path,
    [int]$Width = 0,
    [int]$Height = 0,
    [object]$Frames = $null
) {
    $asset = [ordered]@{
        kind = $Kind
        source = [ordered]@{ kind = "packaged"; path = $Path }
    }
    if ($Width -gt 0 -and $Height -gt 0) {
        $asset.declared_size = [ordered]@{
            width = $Width
            height = $Height
            decoded_bytes = $Width * $Height * 4
        }
    }
    if ($null -ne $Frames) { $asset.frames = $Frames }
    return $asset
}

function New-StateGrid([string]$Id, [string]$Type, [string[]]$States) {
    $nodes = foreach ($state in $States) {
        $node = New-ControlNode $Type "gallery.pair.components.$Id.$state" ([cultureinfo]::InvariantCulture.TextInfo.ToTitleCase($state)) $state
        if ($Type -eq "segmented") {
            $node.options = @(
                [ordered]@{ value = $state; label = New-TextContent ([cultureinfo]::InvariantCulture.TextInfo.ToTitleCase($state)) }
            )
            if ($state -eq "selected") { $node.selected = $state }
        }
        if ($Type -eq "checkbox" -and $state -eq "selected") { $node.checked = $true }
        if ($Type -eq "toggle" -and $state -eq "selected") { $node.on = $true }
        $node
    }
    return New-GridNode "gallery.section.component_$Id" @($nodes) 4
}

$atlasFrames = [ordered]@{}
foreach ($frame in @(
    @("red_circle", 0),
    @("green_square", 32),
    @("blue_diamond", 64),
    @("yellow_cross", 96)
)) {
    $atlasFrames[$frame[0]] = [ordered]@{
        x = $frame[1]
        y = 0
        width = 32
        height = 32
        original_width = 32
        original_height = 32
        pivot = [ordered]@{ x = 0.5; y = 0.5 }
    }
}

$assets = [ordered]@{
    fixture_transparent = New-PackagedAsset "image" "ui/fixtures/visual-foundation/transparent-edge.png" 64 64
    fixture_landscape = New-PackagedAsset "image" "ui/fixtures/visual-foundation/non-square-2x1.png" 128 64
    fixture_nine_slice = New-PackagedAsset "image" "ui/fixtures/visual-foundation/nine-slice-12px.png" 48 48
    fixture_atlas = New-PackagedAsset "atlas" "ui/fixtures/visual-foundation/atlas-four-frames.png" 128 32 $atlasFrames
    fixture_atlas_source = New-PackagedAsset "image" "ui/fixtures/visual-foundation/atlas-four-frames.png" 128 32
    image_dragon_01 = New-PackagedAsset "image" "ui/images/battlepass_bg_dragon01.png" 2640 1440
    image_dragon_02 = New-PackagedAsset "image" "ui/images/battlepass_bg_dragon02.png" 2640 1440
    atlas_day_goal_tap = New-PackagedAsset "image" "ui/atlas/day_goal_tap.png" 126 148
    atlas_day_goal_tap2 = New-PackagedAsset "image" "ui/atlas/day_goal_tap2.png" 126 148
    atlas_puzzle_img1 = New-PackagedAsset "image" "ui/atlas/puzzle_img1.png" 319 64
    atlas_puzzle_icon = New-PackagedAsset "image" "ui/atlas/puzzle_img_icon.png" 80 80
    atlas_puzzle_select = New-PackagedAsset "image" "ui/atlas/puzzle_img_select.png" 477 179
    atlas_puzzle_select1 = New-PackagedAsset "image" "ui/atlas/puzzle_img_select1.png" 39 39
    atlas_puzzle_time = New-PackagedAsset "image" "ui/atlas/puzzle_img_time.png" 51 66
    icon_add = New-PackagedAsset "icon" "ui/icons/add.png" 96 96
    icon_remove = New-PackagedAsset "icon" "ui/icons/remove.png" 96 96
    icon_help = New-PackagedAsset "icon" "ui/icons/help.png" 96 96
    icon_close = New-PackagedAsset "icon" "ui/icons/close.png" 96 96
    icon_loading = New-PackagedAsset "icon" "ui/icons/loading.png" 96 96
    icon_arrow_left = New-PackagedAsset "icon" "ui/icons/arrow-left.png" 96 96
    icon_arrow_right = New-PackagedAsset "icon" "ui/icons/arrow-right.png" 96 96
    icon_full_color = New-PackagedAsset "icon" "ui/icons/full-color-badge.png" 96 96
    icon_missing = New-PackagedAsset "icon" "ui/icons/missing.png" 96 96
    button_add = New-PackagedAsset "image" "ui/icons/add.png" 96 96
    button_remove = New-PackagedAsset "image" "ui/icons/remove.png" 96 96
    button_help = New-PackagedAsset "image" "ui/icons/help.png" 96 96
    button_close = New-PackagedAsset "image" "ui/icons/close.png" 96 96
    button_loading = New-PackagedAsset "image" "ui/icons/loading.png" 96 96
    button_arrow_left = New-PackagedAsset "image" "ui/icons/arrow-left.png" 96 96
    button_arrow_right = New-PackagedAsset "image" "ui/icons/arrow-right.png" 96 96
    button_full_color = New-PackagedAsset "image" "ui/icons/full-color-badge.png" 96 96
    button_missing = New-PackagedAsset "image" "ui/icons/missing.png" 96 96
    gallery_frosted_material = [ordered]@{
        kind = "material"
        source = [ordered]@{ kind = "built_in_material"; material = "frosted_panel_v1" }
    }
}

$fitCards = foreach ($sample in @(
    @("natural", "Natural", "natural", 0.5, 0.5),
    @("stretch", "Stretch", "stretch", 0.5, 0.5),
    @("contain", "Contain", "contain", 0.5, 0.5),
    @("cover", "Cover focus 0 / 1", "cover", 0.0, 1.0)
)) {
    $id = "gallery.pair.image_fit.$($sample[0])"
    $landscapeImage = New-ImageNode "$id.landscape" "fixture_landscape" (New-FitPresentation $sample[2] $sample[3] $sample[4]) 72 44
    $landscapeImage.layout.width = [ordered]@{ percent = 100 }
    $landscapeImage.layout.height = [ordered]@{ percent = 100 }
    $portraitImage = New-ImageNode "$id.portrait" "fixture_landscape" (New-FitPresentation $sample[2] $sample[3] $sample[4]) 44 72
    $portraitImage.layout.width = [ordered]@{ percent = 100 }
    $portraitImage.layout.height = [ordered]@{ percent = 100 }
    $landscape = New-ImageFrame "$id.landscape.frame" $landscapeImage ([ordered]@{ px = 72 }) ([ordered]@{ px = 44 })
    $portrait = New-ImageFrame "$id.portrait.frame" $portraitImage ([ordered]@{ px = 44 }) ([ordered]@{ px = 72 })
    New-ImageFitCard $id $sample[1] $landscape $portrait
}

$fixtureNodes = foreach ($fixture in @(
    @("transparent_edge", "fixture_transparent"),
    @("non_square_2x1", "fixture_landscape"),
    @("nine_slice_12px", "fixture_nine_slice"),
    @("atlas_four_frames", "fixture_atlas_source")
)) {
    $id = "gallery.pair.visual_fixture.$($fixture[0])"
    $image = New-ImageNode $id $fixture[1] (New-FitPresentation "stretch") 96 96
    $image.layout.width = [ordered]@{ percent = 100 }
    $image.layout.height = [ordered]@{ percent = 100 }
    $frame = New-ImageFrame "$id.frame" $image ([ordered]@{ percent = 100 }) "auto" 1.0
    New-GalleryCardNode "$id.card" @($frame)
}

$visualFoundation = New-SectionNode "gallery.section.visual_foundation" "Visual Foundation" "Alpha edge, 2:1 image, nine-slice, and atlas fixtures." @(
    New-TextNode "gallery.section.image_fit.title" "Image Fit" "section_label" "muted"
    New-TextNode "gallery.section.image_fit.description" "Natural, Stretch, Contain, and focused Cover in landscape and portrait frames." "body" "muted"
    New-GridNode "gallery.section.image_fit" @($fitCards) 4
    New-GridNode "gallery.visual_foundation.fixtures" @($fixtureNodes) 4
)

$acceptanceBackground = [ordered]@{
    type = "image"
    id = "gallery.pair.visual_acceptance.background"
    asset = "image_dragon_01"
    presentation = New-FitPresentation "cover"
    layout = [ordered]@{ width = [ordered]@{ percent = 100 }; height = [ordered]@{ percent = 100 } }
}
$acceptanceBackgroundFrame = New-ImageFrame "gallery.pair.visual_acceptance.background.frame" $acceptanceBackground ([ordered]@{ percent = 100 }) "auto" 3.2

$acceptanceFeatureNodes = @(
    [ordered]@{
        type = "container"
        id = "gallery.pair.visual_acceptance.shadow_gradient"
        layout = [ordered]@{
            width = [ordered]@{ percent = 100 }
            min_height = [ordered]@{ px = 88 }
            align_items = "center"
            justify_content = "center"
            padding = [ordered]@{ all = [ordered]@{ px = 12 } }
        }
        style = [ordered]@{ component = "gallery_composite" }
        children = @(New-TextNode "gallery.pair.visual_acceptance.shadow_gradient.label" "Shadow + gradient" "caption")
    }
    [ordered]@{
        type = "container"
        id = "gallery.pair.visual_acceptance.nine_slice.card"
        layout = [ordered]@{ direction = "column"; align_items = "center"; gap = [ordered]@{ px = 6 } }
        children = @(
            New-ImageNode "gallery.pair.visual_acceptance.nine_slice" "fixture_nine_slice" ([ordered]@{
                kind = "nine_slice"; insets = [ordered]@{ left = 12; right = 12; top = 12; bottom = 12 }
            }) 120 72
            New-TextNode "gallery.pair.visual_acceptance.nine_slice.label" "Nine-slice" "caption" "muted"
        )
    }
)

$acceptanceWeightNodes = @(
    [ordered]@{ type = "text"; id = "gallery.pair.visual_acceptance.regular"; content = New-TextContent "Regular"; typography = [ordered]@{ weight = "regular" }; layout = [ordered]@{ width = [ordered]@{ percent = 100 }; min_height = [ordered]@{ px = 24 } }; style = [ordered]@{ text_role = "caption" } }
    [ordered]@{ type = "text"; id = "gallery.pair.visual_acceptance.medium"; content = New-TextContent "Medium"; typography = [ordered]@{ weight = "medium" }; layout = [ordered]@{ width = [ordered]@{ percent = 100 }; min_height = [ordered]@{ px = 24 } }; style = [ordered]@{ text_role = "caption" } }
    [ordered]@{ type = "text"; id = "gallery.pair.visual_acceptance.bold"; content = New-TextContent "Bold"; typography = [ordered]@{ weight = "bold" }; layout = [ordered]@{ width = [ordered]@{ percent = 100 }; min_height = [ordered]@{ px = 24 } }; style = [ordered]@{ text_role = "caption" } }
)

$acceptance = New-SectionNode "gallery.section.visual_acceptance" "High Fidelity Acceptance" "One stable recipe combining production images, scalable surfaces, typography, icons, effects, and states." @(
    $acceptanceBackgroundFrame
    New-GridNode "gallery.visual_acceptance.features" $acceptanceFeatureNodes 2
    New-GridNode "gallery.visual_acceptance.weights" $acceptanceWeightNodes 3
    New-GridNode "gallery.visual_acceptance.states" @(
        (New-IconButtonNode "gallery.pair.visual_acceptance.help" "button_help" "normal" "secondary" "" "Help")
        (New-ControlNode "button" "gallery.pair.visual_acceptance.selected" "Selected" "selected" "secondary")
        (New-ControlNode "button" "gallery.pair.visual_acceptance.loading" "Loading" "loading" "primary")
        (New-ControlNode "button" "gallery.pair.visual_acceptance.disabled" "Disabled" "disabled" "primary")
    ) 4
)
$acceptance.children[1].style.text_role = "caption"

$nineSlicePresentation = [ordered]@{
    kind = "nine_slice"
    insets = [ordered]@{ left = 12; right = 12; top = 12; bottom = 12 }
    center = [ordered]@{ kind = "stretch" }
    sides = [ordered]@{ kind = "stretch" }
    max_corner_scale = 1.0
    max_generated_slices = 256
}
$nineSliceCards = @(
    New-ImageCard "gallery.pair.image_mode.nine_slice.panel" "fixture_nine_slice" $nineSlicePresentation "Panel border 184 x 84" 184 84
    New-ImageCard "gallery.pair.image_mode.nine_slice.small" "fixture_nine_slice" $nineSlicePresentation "Small 72 x 32" 72 32
    New-ImageCard "gallery.pair.image_mode.nine_slice.medium" "fixture_nine_slice" $nineSlicePresentation "Medium 104 x 40" 104 40
    New-ImageCard "gallery.pair.image_mode.nine_slice.large" "fixture_nine_slice" $nineSlicePresentation "Large 144 x 48" 144 48
)
$tilingCards = foreach ($tile in @(
    @("x", "Tile X", 184, 52),
    @("y", "Tile Y", 92, 116),
    @("both", "Tile Both", 184, 116)
)) {
    New-ImageCard "gallery.pair.image_mode.tile_$($tile[0])" "fixture_landscape" ([ordered]@{
        kind = "tiled"; axis = $tile[0]; stretch_value = 0.5; max_repeats = 32
    }) $tile[1] $tile[2] $tile[3]
}
$atlasCards = foreach ($frame in @(
    @("red", "red_circle", "Red circle"),
    @("green", "green_square", "Green square"),
    @("blue", "blue_diamond", "Blue diamond"),
    @("yellow", "yellow_cross", "Yellow cross")
)) {
    New-ImageCard "gallery.pair.image_mode.atlas_$($frame[0])" "fixture_atlas" ([ordered]@{
        kind = "atlas_frame"; frame = $frame[1]; fit = "stretch"
    }) $frame[2] 56 56
}
$imageModes = New-SectionNode "gallery.section.image_modes" "Nine-slice, Tiling, and Atlas Frames" "Scalable borders, bounded texture repetition, and exact frame regions." @(
    New-TextNode "gallery.image_modes.nine_slice" "Nine-slice" "caption" "muted"
    $nineSliceCards[0]
    New-GridNode "gallery.image_modes.nine_slice_grid" @($nineSliceCards[1..3]) 3
    New-TextNode "gallery.image_modes.tiling" "Bounded tiling" "caption" "muted"
    New-GridNode "gallery.section.image_tiling" @($tilingCards) 3
    New-TextNode "gallery.image_modes.atlas" "Atlas frames" "caption" "muted"
    New-GridNode "gallery.section.image_atlas" @($atlasCards) 4
)

$typography = New-SectionNode "gallery.section.typography" "Typography" "Theme roles, real Latin fixture weights, mixed text, wrapping, and bounded overflow." @(
    New-TextNode "gallery.pair.typography.large_title" "Large Title" "title"
    New-TextNode "gallery.pair.typography.section_title" "Section Title" "heading"
    New-TextNode "gallery.pair.typography.subtitle" "Subtitle text" "subtitle"
    New-TextNode "gallery.pair.typography.body" "Body text" "body"
    New-TextNode "gallery.pair.typography.caption" "Caption text" "caption"
    New-TextNode "gallery.pair.typography.button" "Button label role" "button"
    New-TextNode "gallery.typography.weights" "Latin fixture weights" "caption" "muted"
    ([ordered]@{ type = "text"; id = "gallery.pair.typography.regular"; content = New-TextContent "Regular 400 / Aa Bb 0123 !?,."; typography = [ordered]@{ weight = "regular" }; style = [ordered]@{ text_role = "body" } })
    ([ordered]@{ type = "text"; id = "gallery.pair.typography.medium"; content = New-TextContent "Medium 500 / Aa Bb 0123 !?,."; typography = [ordered]@{ weight = "medium" }; style = [ordered]@{ text_role = "body" } })
    ([ordered]@{ type = "text"; id = "gallery.pair.typography.bold"; content = New-TextContent "Bold 700 / Aa Bb 0123 !?,."; typography = [ordered]@{ weight = "bold" }; style = [ordered]@{ text_role = "body" } })
)

$clipText = [ordered]@{
    type = "text"
    id = "gallery.pair.typography.clip"
    content = New-TextContent "Clip / 0123456789 / This text stays intact and the constrained parent clips it."
    typography = [ordered]@{ wrap = "no_wrap"; overflow = "clip" }
    style = [ordered]@{ text_role = "body" }
}
$clipFrame = [ordered]@{
    type = "container"
    id = "gallery.pair.typography.clip.frame"
    layout = [ordered]@{
        width = [ordered]@{ px = 280 }
        height = [ordered]@{ px = 32 }
        overflow = [ordered]@{ x = "clip"; y = "clip" }
    }
    children = @($clipText)
}

$typographyOverflow = New-SectionNode "gallery.section.typography_overflow" "Mixed text and overflow states" "" @(
    New-TextNode "gallery.pair.typography.mixed" "MyBevy 中文混排 2026，标点：！？；ABC-123"
    ([ordered]@{ type = "text"; id = "gallery.pair.typography.long_word"; content = New-TextContent "InteroperabilityWithoutWhitespaceMustStillWrapAtACharacterBoundary"; typography = [ordered]@{ wrap = "word_or_character" }; layout = [ordered]@{ width = [ordered]@{ percent = 100 }; max_width = [ordered]@{ px = 420 } }; style = [ordered]@{ text_role = "body" } })
    ([ordered]@{ type = "text"; id = "gallery.pair.typography.long_cjk"; content = New-TextContent "这是一段用于验证超长中文在紧凑容器中按字符安全换行且不会覆盖相邻内容的文字。"; typography = [ordered]@{ wrap = "character" }; layout = [ordered]@{ width = [ordered]@{ percent = 100 }; max_width = [ordered]@{ px = 520 } }; style = [ordered]@{ text_role = "body" } })
    $clipFrame
    ([ordered]@{ type = "text"; id = "gallery.pair.typography.ellipsis"; content = New-TextContent "Ellipsis / 中文和English按字素簇安全截断 / 0123456789"; typography = [ordered]@{ wrap = "no_wrap"; max_lines = 1; overflow = "ellipsis" }; layout = [ordered]@{ width = [ordered]@{ percent = 100 } }; style = [ordered]@{ text_role = "body" } })
)

$typographyBoundary = New-SectionNode "gallery.section.typography_boundary" "Alignment and missing glyph" "" @(
    ([ordered]@{ type = "text"; id = "gallery.pair.typography.centered"; content = New-TextContent "Centered / punctuation ！？，. / 2026"; typography = [ordered]@{ alignment = "center" }; layout = [ordered]@{ width = [ordered]@{ percent = 100 } }; style = [ordered]@{ text_role = "body" } })
    New-TextNode "gallery.pair.typography.missing_glyph" "Missing glyph sample: ? becomes explicit question mark" "caption" "muted"
)

$buttonNodes = foreach ($button in @(
    @("primary", "Primary", "normal", "primary"),
    @("secondary", "Secondary", "normal", "secondary"),
    @("focused", "Focused", "focused", "primary"),
    @("selected", "Selected", "selected", "secondary"),
    @("loading", "Loading", "loading", "primary"),
    @("disabled", "Disabled", "disabled", "primary"),
    @("unavailable", "Unavailable", "disabled", "secondary"),
    @("action", "Action", "normal", "primary")
)) {
    New-ControlNode "button" "gallery.pair.button.$($button[0])" $button[1] $button[2] $button[3]
}
$buttons = New-SectionNode "gallery.section.buttons" "Buttons" "" @(
    New-GridNode "gallery.buttons.grid" @($buttonNodes) 4
)

$iconOnlyNodes = foreach ($icon in @(
    @("add", "Add", "button_add"),
    @("remove", "Remove", "button_remove"),
    @("help", "Help", "button_help"),
    @("close", "Close", "button_close"),
    @("loading", "Loading", "button_loading")
)) {
    New-IconButtonNode "gallery.pair.icon.$($icon[0])" $icon[2] "normal" "secondary" "" $icon[1]
}
$previousIconButton = New-IconButtonNode "gallery.pair.icon.previous" "button_arrow_left" "normal" "secondary" "Previous"
$nextIconButton = New-IconButtonNode "gallery.pair.icon.next" "button_arrow_right" "normal" "secondary" "Next"
$nextIconButton.style = [ordered]@{ role = "icon_trailing" }
$labeledIconNodes = @($previousIconButton, $nextIconButton)
$imageButtonSamples = @(
    New-IconSample "gallery.pair.icon.tintable" (New-IconButtonNode "gallery.pair.icon.tintable" "button_help" "normal" "secondary" "" "Tintable") "Tintable"
    New-IconSample "gallery.pair.icon.full_color" (New-IconButtonNode "gallery.pair.icon.full_color" "button_full_color" "normal" "secondary" "" "Full color") "Full color"
    New-IconSample "gallery.pair.icon.missing" (New-IconButtonNode "gallery.pair.icon.missing" "button_missing" "normal" "secondary" "" "Missing") "Missing"
)
$icons = New-SectionNode "gallery.section.icons" "Icon and Image Buttons" "Asset icons, labeled placement, tint policy, and a visible missing placeholder." @(
    New-TextNode "gallery.icons.icon_only" "Icon only" "caption" "muted"
    New-GridNode "gallery.icons.icon_only_grid" @($iconOnlyNodes) 5
    New-TextNode "gallery.icons.labeled" "Icon and label" "caption" "muted"
    New-GridNode "gallery.icons.labeled_grid" $labeledIconNodes 2
    New-GridNode "gallery.icons.image_samples" $imageButtonSamples 3
)

$iconStateNodes = foreach ($state in @("idle", "hovered", "pressed", "focused", "selected", "disabled", "loading")) {
    $componentState = if ($state -eq "idle") { "normal" } else { $state }
    $id = "gallery.pair.icon_state.$state"
    New-IconSample $id (New-IconButtonNode $id "button_help" $componentState "secondary" "" ([cultureinfo]::InvariantCulture.TextInfo.ToTitleCase($state))) ([cultureinfo]::InvariantCulture.TextInfo.ToTitleCase($state))
}
$iconStates = New-SectionNode "gallery.section.icon_states" "Icon Button States" "Pointer, focus, selection, disabled, and loading use one state priority." @(
    New-GridNode "gallery.icon_states.grid" @($iconStateNodes) 7
)

$styleNodes = foreach ($style in @(
    @("global", "Global default", "gallery_panel"),
    @("parent", "Parent scope", "gallery_parent"),
    @("nested", "Nested scope", "gallery_nested"),
    @("restored", "Outside scope / restored", "gallery_panel")
)) {
    [ordered]@{
        type = "container"
        id = "gallery.pair.style.$($style[0])"
        style = [ordered]@{ component = $style[2] }
        layout = [ordered]@{ padding = [ordered]@{ all = [ordered]@{ px = 12 } } }
        children = @(New-TextNode "gallery.pair.style.$($style[0]).label" $style[1] "caption")
    }
}
$styleNodes += New-ControlNode "button" "gallery.pair.style.selected_button" "Selected persists" "selected" "secondary"
$styleNodes += [ordered]@{ type = "image_button"; id = "gallery.pair.style.selected_icon"; asset = "button_help"; component = New-Component "Selected scoped icon" "selected" "secondary" }
$stylesSection = New-SectionNode "gallery.section.style_scopes" "Scoped Styles" "Global, inherited, nested, and restored style resolution." @(
    New-GridNode "gallery.styles.grid" @($styleNodes) 4
)

$effectNodes = foreach ($effect in @(
    @("box_shadow", "Layered box shadow", "gallery_shadow"),
    @("text_shadow", "Text shadow", "gallery_text_shadow"),
    @("gradient", "Linear + border gradient", "gallery_gradient"),
    @("composite", "Rounded clipped composite", "gallery_composite"),
    @("material", "Material fallback", "gallery_frosted")
)) {
    [ordered]@{
        type = "container"
        id = "gallery.pair.effect.$($effect[0])"
        layout = [ordered]@{ width = [ordered]@{ px = 180 }; height = [ordered]@{ px = 88 }; padding = [ordered]@{ all = [ordered]@{ px = 10 } } }
        style = [ordered]@{ component = $effect[2] }
        children = @(New-TextNode "gallery.pair.effect.$($effect[0]).label" $effect[1] "caption")
    }
}
$effects = New-SectionNode "gallery.section.effects" "Effects" "Bounded shadows, gradients, outline, clipping, and visible material fallback." @(
    New-GridNode "gallery.effects.grid" @($effectNodes) 3
)

$animationNodes = foreach ($animation in @(
    @("control", "Control"),
    @("page_entry", "Page entry"),
    @("dialog_entry", "Dialog entry"),
    @("dialog_exit", "Dialog exit"),
    @("loading_loop", "Loading loop"),
    @("layout_size", "Layout size"),
    @("color_transition", "Color transition"),
    @("alpha_transition", "Alpha transition")
)) {
    $id = "gallery.pair.animation.$($animation[0])"
    if ($animation[0] -eq "control") {
        New-ControlNode "button" $id $animation[1] "normal" "primary"
    } else {
        $animationStyle = switch ($animation[0]) {
            "layout_size" { "gallery_animation_layout" }
            "color_transition" { "gallery_animation_color" }
            "alpha_transition" { "gallery_animation_alpha" }
            default { "gallery_animation_tile" }
        }
        $animationLayout = [ordered]@{
            width = [ordered]@{ percent = 100 }
            min_height = [ordered]@{ px = 88 }
            align_items = "center"
            justify_content = "center"
            padding = [ordered]@{ all = [ordered]@{ px = 10 } }
        }
        if ($animation[0] -eq "layout_size") {
            $animationLayout.width = [ordered]@{ px = 96 }
            $animationLayout.height = [ordered]@{ px = 72 }
            $animationLayout.min_width = [ordered]@{ px = 96 }
            $animationLayout.min_height = [ordered]@{ px = 72 }
            $animationLayout.justify_self = "center"
        }
        [ordered]@{
            type = "container"
            id = $id
            layout = $animationLayout
            style = [ordered]@{ component = $animationStyle }
            children = @(New-TextNode "$id.label" $animation[1] "caption")
        }
    }
}
$animations = New-SectionNode "gallery.section.animations" "Animation and Motion" "Transform-first transitions, explicit layout motion, and deterministic policy states." @(
    New-GridNode "gallery.animations.grid" @($animationNodes) 4
)

$badgeNodes = foreach ($state in @("normal", "selected", "disabled", "loading", "empty", "error")) {
    New-ControlNode "badge" "gallery.pair.components.badge.$state" ([cultureinfo]::InvariantCulture.TextInfo.ToTitleCase($state)) $state "info"
}
$progressValues = [ordered]@{ normal = 0.72; disabled = 0.34; loading = 0.62; empty = 0.0; error = 0.0 }
$progressLabels = [ordered]@{ normal = "72%"; disabled = "34%"; loading = "Loading"; empty = "Empty"; error = "Error" }
$progressNodes = foreach ($state in $progressValues.Keys) {
    [ordered]@{
        type = "progress"
        id = "gallery.pair.components.progress.$state"
        value = $progressValues[$state]
        component = New-Component $progressLabels[$state] $state "info"
    }
}
$tabNodes = foreach ($state in @("normal", "hovered", "pressed", "focused", "selected", "disabled", "loading")) {
    $node = New-ControlNode "tab" "gallery.pair.components.tab.$state" ([cultureinfo]::InvariantCulture.TextInfo.ToTitleCase($state)) $state
    $node.value = $state
    if ($state -in @("normal", "selected")) {
        $node.on_change = [ordered]@{ action = "gallery.control_changed" }
    }
    $node
}
$dropdownOptions = @(
    [ordered]@{ value = "north"; label = New-TextContent "North Realm" }
    [ordered]@{ value = "locked"; label = New-TextContent "Locked Region"; disabled = $true }
    [ordered]@{ value = "long"; label = New-TextContent "A deliberately long region name that wraps without resizing the control" }
    [ordered]@{ value = "south"; label = New-TextContent "South Realm" }
)
$dropdownLabels = [ordered]@{
    normal = "Choose a region"; hovered = "Hovered"; pressed = "Pressed"; focused = "Focused";
    selected = "Selected"; disabled = "Disabled"; loading = "Loading"; empty = "Empty"; error = "Error"
}
$dropdownNodes = foreach ($state in $dropdownLabels.Keys) {
    $id = if ($state -eq "normal") { "gallery.select" } else { "gallery.pair.components.dropdown.$state" }
    $node = [ordered]@{
        type = "select"
        id = $id
        options = $dropdownOptions
        component = New-Component "" $state "default" "" $dropdownLabels[$state]
    }
    if ($state -eq "selected") { $node.selected = "north" }
    if ($state -eq "normal") {
        $node.on_change = [ordered]@{ action = "gallery.control_changed" }
    }
    $node
}
$tooltipNodes = @(
    [ordered]@{
        type = "tooltip"; id = "gallery.tooltip"; tone = "standard"
        component = [ordered]@{
            states = @("normal")
            slots = [ordered]@{ body = New-Slot "Tooltip text stays inside the viewport and follows its owner." }
            children = @(New-ControlNode "button" "gallery.tooltip.target" "Hover or focus" "normal" "secondary")
        }
    }
    [ordered]@{
        type = "tooltip"; id = "gallery.pair.components.tooltip.error"; tone = "error"
        component = [ordered]@{
            states = @("error")
            slots = [ordered]@{ body = New-Slot "The requested data could not be loaded." }
            children = @(New-ControlNode "button" "gallery.tooltip.error.target" "Error hint" "normal" "secondary")
        }
    }
    [ordered]@{
        type = "tooltip"; id = "gallery.pair.components.tooltip.disabled"; tone = "standard"
        component = [ordered]@{
            states = @("disabled")
            slots = [ordered]@{ body = New-Slot "Disabled controls may still explain why an action is unavailable." }
            children = @(New-ControlNode "button" "gallery.tooltip.disabled.target" "Disabled owner" "disabled" "secondary")
        }
    }
)
$matrixStates = @("normal", "hovered", "pressed", "focused", "selected", "disabled", "loading", "error")
$tabsLayout = New-ContainerNode "gallery.components.tabs_grid" @($tabNodes) "row" $true
$tabsLayout.layout.width = [ordered]@{ percent = 100 }
$tabsLayout.layout.gap = [ordered]@{ px = 0 }
$components = New-SectionNode "gallery.section.components" "Component States" "Reusable component states, stable geometry, overlays, and long-text boundaries." @(
    New-TextNode "gallery.components.badge" "Badge" "section_label" "muted"
    New-GridNode "gallery.components.badges" @($badgeNodes) 6
    New-TextNode "gallery.components.progress" "Progress" "section_label" "muted"
    New-GridNode "gallery.components.progresses" @($progressNodes) 3
    New-TextNode "gallery.components.tabs" "Tabs" "section_label" "muted"
    $tabsLayout
    New-TextNode "gallery.components.dropdown" "Dropdown" "section_label" "muted"
    New-GridNode "gallery.section.component_dropdown" @($dropdownNodes) 3
    New-TextNode "gallery.components.tooltip" "Tooltip" "section_label" "muted"
    New-GridNode "gallery.section.component_tooltip" $tooltipNodes 3
    New-TextNode "gallery.components.selection_states" "Checkbox, Toggle, and Segmented States" "section_label" "muted"
    New-StateGrid "checkboxes" "checkbox" $matrixStates
    New-StateGrid "toggles" "toggle" $matrixStates
    New-StateGrid "segmented" "segmented" $matrixStates
)

$selectedCheckbox = [ordered]@{ type = "checkbox"; id = "gallery.pair.selection.checkbox.checked"; checked = $true; component = New-Component "Checked" "selected"; on_change = [ordered]@{ action = "gallery.control_changed" } }
$selectedToggle = [ordered]@{ type = "toggle"; id = "gallery.pair.selection.toggle.on"; on = $true; component = New-Component "Toggle On" "selected"; on_change = [ordered]@{ action = "gallery.control_changed" } }
$selectionSegmented = [ordered]@{
    type = "segmented"; id = "gallery.pair.selection.segmented"; selected = "medium"
    options = @(
        [ordered]@{ value = "small"; label = New-TextContent "Small" }
        [ordered]@{ value = "medium"; label = New-TextContent "Medium" }
        [ordered]@{ value = "large"; label = New-TextContent "Large"; disabled = $true }
    )
    component = New-Component ""
    on_change = [ordered]@{ action = "gallery.control_changed" }
}
$selectionControls = @(
    New-ControlNode "checkbox" "gallery.pair.selection.checkbox.unchecked" "Unchecked"
    $selectedCheckbox
    New-ControlNode "checkbox" "gallery.pair.selection.checkbox.disabled" "Disabled" "disabled"
    New-ControlNode "toggle" "gallery.pair.selection.toggle.off" "Toggle Off"
    $selectedToggle
    New-ControlNode "toggle" "gallery.pair.selection.toggle.disabled" "Toggle Disabled" "disabled"
    $selectionSegmented
)
$selection = New-SectionNode "gallery.section.selection" "Selection Controls" "" @(
    New-GridNode "gallery.selection.grid" $selectionControls 3
)

$numeric = New-SectionNode "gallery.section.numeric" "Numeric Controls" "" @(
    ([ordered]@{ type = "slider"; id = "gallery.pair.numeric.volume"; value = 64; min = 0; max = 100; component = New-Component "Volume"; on_change = [ordered]@{ action = "gallery.control_changed" } })
    ([ordered]@{ type = "slider"; id = "gallery.pair.numeric.slider_disabled"; value = 30; min = 0; max = 100; component = New-Component "Disabled Slider" "disabled" })
    ([ordered]@{ type = "stepper"; id = "gallery.pair.numeric.players"; value = 4; min = 1; max = 8; step = 1; component = New-Component "Players"; on_change = [ordered]@{ action = "gallery.control_changed" } })
    ([ordered]@{ type = "stepper"; id = "gallery.pair.numeric.stepper_disabled"; value = 2; min = 1; max = 8; step = 1; component = New-Component "Disabled Stepper" "disabled" })
)

$inputs = @(
    [ordered]@{ id = "player_name"; placeholder = "Player name"; value = "Pilot 01"; helper = "Shown to other players."; state = "normal"; max = 64; readonly = $false; error = "" }
    [ordered]@{ id = "required"; placeholder = "Required"; value = ""; helper = "Required fields validate empty values."; state = "error"; max = 64; readonly = $false; error = "This field is required." }
    [ordered]@{ id = "error"; placeholder = "Error state"; value = "bad-code"; helper = ""; state = "error"; max = 8; readonly = $false; error = "Use 4-8 letters or numbers." }
    [ordered]@{ id = "note"; placeholder = "Type a note"; value = ""; helper = "Limited to 12 characters."; state = "normal"; max = 12; readonly = $false; error = "" }
    [ordered]@{ id = "readonly"; placeholder = "Read only"; value = "Readonly sample"; helper = "Readonly keeps focus but does not edit."; state = "normal"; max = 64; readonly = $true; error = "" }
    [ordered]@{ id = "disabled"; placeholder = "Disabled"; value = "Disabled sample"; helper = ""; state = "disabled"; max = 64; readonly = $false; error = "Disabled visual state wins over error." }
    [ordered]@{ id = "short_code"; placeholder = "Max 6 chars"; value = "ABC"; helper = "Required, max 6 characters."; state = "normal"; max = 6; readonly = $false; error = "" }
    [ordered]@{ id = "empty"; placeholder = "Empty input"; value = ""; helper = "Optional empty field."; state = "normal"; max = 64; readonly = $false; error = "" }
)
$inputNodes = foreach ($input in $inputs) {
    $node = [ordered]@{
        type = "text_input"
        id = "gallery.pair.input.$($input.id)"
        value = $input.value
        max_chars = $input.max
        readonly = $input.readonly
        component = New-Component $input.placeholder $input.state "default" $input.helper $input.placeholder $input.error
    }
    if ($input.id -eq "player_name") {
        $node.component.bindings = [ordered]@{
            value = [ordered]@{
                binding_path = "gallery.player_name"
                direction = "two_way"
            }
        }
        $node.on_change = [ordered]@{ action = "gallery.control_changed" }
        $node.on_submit = [ordered]@{ action = "gallery.control_changed" }
    }
    $node
}
$inputsColumn = New-ContainerNode "gallery.inputs.column" @($inputNodes)
$inputsColumn.layout.gap = [ordered]@{ px = 6 }
$inputsSection = New-SectionNode "gallery.section.inputs" "Inputs" "" @($inputsColumn)

$bindingStatus = [ordered]@{
    type = "text"
    id = "gallery.pair.binding.status"
    content = [ordered]@{ binding_path = "gallery.status"; fallback = "Waiting for binding update." }
}
$bindingUpdate = New-ControlNode "button" "gallery.pair.binding.update" "Update Binding" "normal" "secondary"
$bindingUpdate.on_click = [ordered]@{
    action = "gallery.set_status"
    params = [ordered]@{
        value = [ordered]@{
            kind = "binding"
            value = [ordered]@{ kind = "string"; value = "Bound text updated" }
        }
    }
}
$binding = New-SectionNode "gallery.section.binding" "Binding Sample" "The controls below are driven by UiBindingValues." @(
    $bindingStatus
    New-TextNode "gallery.pair.binding.notice" "This prompt is controlled by a bool visibility binding." "body"
    New-ControlNode "button" "gallery.pair.binding.bound_button" "Bound Button" "normal" "secondary"
    $bindingUpdate
)

$overlayButtons = foreach ($overlay in @(
    @("toast", "Show Toast"),
    @("loading", "Loading"),
    @("cancelable", "Cancelable"),
    @("hide", "Hide"),
    @("confirm", "Show Confirm"),
    @("floating", "Show Floating"),
    @("close_top", "Close Top")
)) {
    New-ControlNode "button" "gallery.pair.overlay.$($overlay[0])" $overlay[1] "normal" "secondary"
}
$overlays = New-SectionNode "gallery.section.overlays" "Overlays" "" @(
    New-GridNode "gallery.overlays.grid" @($overlayButtons) 5
)

$atlasSourceNodes = foreach ($source in @(
    @("day_goal_tap", "atlas_day_goal_tap", "day_goal_tap.png"),
    @("day_goal_tap2", "atlas_day_goal_tap2", "day_goal_tap2.png"),
    @("puzzle_img1", "atlas_puzzle_img1", "puzzle_img1.png"),
    @("puzzle_img_icon", "atlas_puzzle_icon", "puzzle_img_icon.png"),
    @("puzzle_img_select", "atlas_puzzle_select", "puzzle_img_select.png"),
    @("puzzle_img_select1", "atlas_puzzle_select1", "puzzle_img_select1.png"),
    @("puzzle_img_time", "atlas_puzzle_time", "puzzle_img_time.png")
)) {
    New-ImageCard "gallery.pair.atlas_source.$($source[0])" $source[1] (New-FitPresentation "stretch") $source[2] 96 96
}
$imageCards = @(
    New-ImageCard "gallery.pair.images.dragon_01" "image_dragon_01" (New-FitPresentation "stretch") "ui/images/battlepass_bg_dragon01.png" 320 175
    New-ImageCard "gallery.pair.images.dragon_02" "image_dragon_02" (New-FitPresentation "stretch") "ui/images/battlepass_bg_dragon02.png" 320 175
)
$images = New-SectionNode "gallery.section.images" "Images" "Regular packaged UI images loaded from assets/ui/images." @(
    New-GridNode "gallery.images.grid" $imageCards 2
    New-TextNode "gallery.section.atlas_sources.title" "Atlas Source Images" "section_label" "muted"
    New-TextNode "gallery.section.atlas_sources.description" "Source PNGs only; this is not a formal atlas frame preview." "body"
    New-GridNode "gallery.section.atlas_sources" @($atlasSourceNodes) 6
)

$stressNodes = for ($index = 1; $index -le 24; $index++) {
    $state = @("Ready", "Waiting", "Done")[($index - 1) % 3]
    $number = $index.ToString("00", [cultureinfo]::InvariantCulture)
    New-ContainerNode "gallery.pair.stress.item_$number" @(
        New-TextNode "gallery.pair.stress.item_$number.title" "Item $number" "caption"
        New-TextNode "gallery.pair.stress.item_$number.state" $state "caption"
        New-ControlNode "button" "gallery.pair.stress.item_$number.inspect" "Inspect" "normal" "secondary"
    ) "column" $false $true
}
$stress = New-SectionNode "gallery.section.stress" "Stress Sample" "Static list for observing node and text counts in F3." @(
    New-GridNode "gallery.stress.grid" @($stressNodes) 3
)

$galleryHeader = New-ContainerNode "gallery.header" @(
    New-TextNode "gallery.title" "UI Gallery (Generated)" "title"
    New-ControlNode "button" "gallery.nav.lobby" "Lobby" "normal" "secondary"
) "row" $true
$galleryHeader.layout.width = [ordered]@{ percent = 100 }
$galleryHeader.layout.max_width = [ordered]@{ px = 760 }
$galleryHeader.layout.align_self = "center"
$galleryHeader.layout.align_items = "center"
$galleryHeader.layout.justify_content = "space_between"

$gallerySections = @(
    $visualFoundation
    $acceptance
    $imageModes
    $typography
    $typographyOverflow
    $typographyBoundary
    $buttons
    $icons
    $iconStates
    $stylesSection
    $effects
    $animations
    $components
    $selection
    $numeric
    $inputsSection
    $binding
    $overlays
    $images
    $stress
)
$galleryScroll = [ordered]@{
    type = "scroll"
    id = "gallery.root"
    row_gap = 18
    block_lower = $true
    layout = [ordered]@{
        width = [ordered]@{ percent = 100 }
        min_height = [ordered]@{ px = 0 }
        flex_grow = 1.0
    }
    component = [ordered]@{ children = $gallerySections }
}
$galleryPage = [ordered]@{
    type = "container"
    id = "gallery.page"
    layout = [ordered]@{
        width = [ordered]@{ percent = 100 }
        height = [ordered]@{ percent = 100 }
        direction = "column"
        gap = [ordered]@{ px = 18 }
        padding = [ordered]@{
            all = [ordered]@{ px = 24 }
            top = [ordered]@{ px = 32 }
        }
    }
    style = [ordered]@{ component = "gallery_screen"; role = "page" }
    children = @($galleryHeader, $galleryScroll)
}

$document = [ordered]@{
    schema_version = 1
    document_id = "gallery.declarative"
    metadata = [ordered]@{
        title = "Declarative UI Gallery full parity baseline"
        budget_profile = "mobile_baseline_v1"
    }
    assets = $assets
    bindings = [ordered]@{
        "gallery.player_name" = [ordered]@{
            scope = "local"
            value_type = [ordered]@{ kind = "string" }
            default = [ordered]@{ kind = "string"; value = "Pilot 01" }
            missing = "use_default"
        }
        "gallery.status" = [ordered]@{
            scope = "local"
            value_type = [ordered]@{ kind = "string" }
            default = [ordered]@{ kind = "string"; value = "Waiting for binding update." }
            missing = "use_default"
        }
    }
    tokens = [ordered]@{
        gallery_screen_surface = [ordered]@{ kind = "color"; value = "#0d141cff" }
        gallery_panel_surface = [ordered]@{ kind = "color"; value = "#1a2129f0" }
        gallery_panel_border = [ordered]@{ kind = "color"; value = "#38474fff" }
        gallery_secondary_surface = [ordered]@{ kind = "color"; value = "#293038ff" }
        gallery_parent_surface = [ordered]@{ kind = "color"; value = "#213747ff" }
        gallery_nested_surface = [ordered]@{ kind = "color"; value = "#143831ff" }
        gallery_radius = [ordered]@{ kind = "number"; value = 8.0 }
        gallery_button_radius = [ordered]@{ kind = "number"; value = 6.0 }
    }
    styles = [ordered]@{
        gallery_screen = [ordered]@{ properties = [ordered]@{
            background = [ordered]@{ kind = "solid"; color = [ordered]@{ kind = "token"; token = "gallery_screen_surface" } }
        } }
        gallery_panel = [ordered]@{ properties = [ordered]@{
            background = [ordered]@{ kind = "solid"; color = [ordered]@{ kind = "token"; token = "gallery_panel_surface" } }
            border = [ordered]@{ width = [ordered]@{ kind = "literal"; value = 1.0 }; color = [ordered]@{ kind = "token"; token = "gallery_panel_border" } }
            corner_radius = [ordered]@{ top_left = [ordered]@{ kind = "token"; token = "gallery_radius" }; top_right = [ordered]@{ kind = "token"; token = "gallery_radius" }; bottom_right = [ordered]@{ kind = "token"; token = "gallery_radius" }; bottom_left = [ordered]@{ kind = "token"; token = "gallery_radius" } }
        } }
        gallery_parent = [ordered]@{ extends = "gallery_panel"; properties = [ordered]@{ background = [ordered]@{ kind = "solid"; color = [ordered]@{ kind = "token"; token = "gallery_parent_surface" } } } }
        gallery_nested = [ordered]@{ extends = "gallery_parent"; properties = [ordered]@{ background = [ordered]@{ kind = "solid"; color = [ordered]@{ kind = "token"; token = "gallery_nested_surface" } } } }
        gallery_image_frame = [ordered]@{ properties = [ordered]@{
            corner_radius = [ordered]@{ top_left = [ordered]@{ kind = "token"; token = "gallery_radius" }; top_right = [ordered]@{ kind = "token"; token = "gallery_radius" }; bottom_right = [ordered]@{ kind = "token"; token = "gallery_radius" }; bottom_left = [ordered]@{ kind = "token"; token = "gallery_radius" } }
        } }
        gallery_image_card = [ordered]@{ properties = [ordered]@{
            background = [ordered]@{ kind = "solid"; color = [ordered]@{ kind = "token"; token = "gallery_secondary_surface" } }
            border = [ordered]@{ width = [ordered]@{ kind = "literal"; value = 1.0 }; color = [ordered]@{ kind = "token"; token = "gallery_panel_border" } }
            corner_radius = [ordered]@{ top_left = [ordered]@{ kind = "token"; token = "gallery_button_radius" }; top_right = [ordered]@{ kind = "token"; token = "gallery_button_radius" }; bottom_right = [ordered]@{ kind = "token"; token = "gallery_button_radius" }; bottom_left = [ordered]@{ kind = "token"; token = "gallery_button_radius" } }
        } }
        gallery_animation_tile = [ordered]@{ extends = "gallery_image_card"; properties = [ordered]@{ background = [ordered]@{ kind = "solid"; color = [ordered]@{ kind = "literal"; value = "#1f2e33ff" } } } }
        gallery_animation_layout = [ordered]@{ extends = "gallery_image_card"; properties = [ordered]@{ background = [ordered]@{ kind = "solid"; color = [ordered]@{ kind = "literal"; value = "#29263bff" } } } }
        gallery_animation_color = [ordered]@{ extends = "gallery_image_card"; properties = [ordered]@{ background = [ordered]@{ kind = "solid"; color = [ordered]@{ kind = "literal"; value = "#147a6eff" } } } }
        gallery_animation_alpha = [ordered]@{ extends = "gallery_image_card"; properties = [ordered]@{ background = [ordered]@{ kind = "solid"; color = [ordered]@{ kind = "literal"; value = "#2e78ab59" } } } }
        gallery_shadow = [ordered]@{ extends = "gallery_panel"; properties = [ordered]@{ shadows = @([ordered]@{ color = [ordered]@{ kind = "literal"; value = "#00000088" }; x_offset = [ordered]@{ kind = "literal"; value = 0.0 }; y_offset = [ordered]@{ kind = "literal"; value = 5.0 }; blur = [ordered]@{ kind = "literal"; value = 12.0 }; spread = [ordered]@{ kind = "literal"; value = 0.0 } }) } }
        gallery_text_shadow = [ordered]@{ extends = "gallery_panel"; properties = [ordered]@{ shadows = @([ordered]@{ color = [ordered]@{ kind = "literal"; value = "#000000cc" }; x_offset = [ordered]@{ kind = "literal"; value = 2.0 }; y_offset = [ordered]@{ kind = "literal"; value = 2.0 }; blur = [ordered]@{ kind = "literal"; value = 4.0 }; spread = [ordered]@{ kind = "literal"; value = 0.0 } }) } }
        gallery_gradient = [ordered]@{ extends = "gallery_panel"; properties = [ordered]@{ background = [ordered]@{ kind = "linear_gradient"; angle_degrees = 25.0; stops = @([ordered]@{ position = 0.0; color = [ordered]@{ kind = "literal"; value = "#176b63ff" } }, [ordered]@{ position = 1.0; color = [ordered]@{ kind = "literal"; value = "#cc8b31ff" } }) } } }
        gallery_composite = [ordered]@{ extends = "gallery_gradient"; properties = [ordered]@{ shadows = @([ordered]@{ color = [ordered]@{ kind = "literal"; value = "#00000088" }; x_offset = [ordered]@{ kind = "literal"; value = 0.0 }; y_offset = [ordered]@{ kind = "literal"; value = 5.0 }; blur = [ordered]@{ kind = "literal"; value = 12.0 }; spread = [ordered]@{ kind = "literal"; value = 0.0 } }) } }
        gallery_frosted = [ordered]@{ extends = "gallery_panel"; properties = [ordered]@{ material = [ordered]@{ asset = "gallery_frosted_material"; parameters = [ordered]@{ kind = "frosted_panel_v1"; blur_px = [ordered]@{ kind = "literal"; value = 14.0 }; opacity = [ordered]@{ kind = "literal"; value = 0.82 }; tint = [ordered]@{ kind = "literal"; value = "#668b9acc" } } } } }
    }
    root = $galleryPage
    responsive = @(
        [ordered]@{
            id = "compact_portrait"
            when = [ordered]@{ width_class = "compact"; orientation = "portrait" }
            overrides = @([ordered]@{ node_id = "gallery.page"; set = [ordered]@{ layout = [ordered]@{ padding = [ordered]@{ all = [ordered]@{ px = 14 } }; gap = [ordered]@{ px = 10 } } } })
        }
        [ordered]@{
            id = "expanded_landscape"
            when = [ordered]@{ width_class = "expanded"; orientation = "landscape" }
            overrides = @([ordered]@{ node_id = "gallery.page"; set = [ordered]@{ layout = [ordered]@{ padding = [ordered]@{ all = [ordered]@{ px = 24 }; top = [ordered]@{ px = 32 } }; gap = [ordered]@{ px = 18 } } } })
        }
    )
}

Set-GalleryNodeI18n $document.root
$json = $document | ConvertTo-Json -Depth 100 -Compress
[System.IO.File]::WriteAllText($outputPath, "$json`n", [System.Text.UTF8Encoding]::new($false))
Write-Output "Generated $outputPath"
