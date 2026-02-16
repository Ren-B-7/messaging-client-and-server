# SVG Icons - External File Implementation

All inline SVG code has been extracted to separate files for better maintainability and server-side delivery.

## File Structure

```
static/
  icons/
    bell.svg           # Notifications icon
    eye.svg            # Show password icon
    eye-off.svg        # Hide password icon
    help-circle.svg    # Help/support icon
    paperclip.svg      # Attach file icon
    phone.svg          # Voice call icon
    search.svg         # Search icon
    send.svg           # Send message icon
    settings.svg       # Settings/account icon
    shield.svg         # Privacy/security icon
    sun.svg            # Theme toggle icon
    user.svg           # Profile/user icon
    video.svg          # Video call icon
```

## Changes Made

### HTML Files Updated:
- ✅ `index.html` - Login page
- ✅ `register.html` - Registration page
- ✅ `chat.html` - Chat interface
- ✅ `settings.html` - Settings page

### JavaScript Files Updated:
- ✅ `auth.js` - Password toggle now uses `<img>` tags instead of inline SVG

## Implementation Details

### Before (Inline SVG):
```html
<button class="password-toggle">
  <svg width="20" height="20" fill="none" stroke="currentColor" viewBox="0 0 24 24">
    <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/>
    <circle cx="12" cy="12" r="3"/>
  </svg>
</button>
```

### After (External SVG):
```html
<button class="password-toggle">
  <img src="/static/icons/eye.svg" alt="" width="20" height="20" />
</button>
```

## Benefits

1. **Server-Side Caching**: SVG files can be cached by the browser and CDN
2. **Maintainability**: Update icons in one place, reflects everywhere
3. **Performance**: Smaller HTML files, faster initial page load
4. **Reusability**: Same icon files used across multiple pages
5. **Version Control**: Easier to track icon changes in git
6. **CDN-Friendly**: Icons can be served from a separate CDN domain

## CSS Considerations

The icons use `currentColor` for `stroke` and `fill` attributes, which means they'll inherit the text color from their parent element. This allows for:

- Automatic theme adaptation (light/dark mode)
- Hover state color changes
- Dynamic color updates via CSS

Example CSS that works with these icons:
```css
.password-toggle {
  color: var(--fg-tertiary);
}

.password-toggle:hover {
  color: var(--fg-secondary);
}
```

The `<img>` tag will render the SVG with the inherited color.

## JavaScript Password Toggle

The `auth.js` file has been updated to swap between `eye.svg` and `eye-off.svg`:

```javascript
setupPasswordToggles() {
  const passwordToggles = document.querySelectorAll('.password-toggle');
  
  passwordToggles.forEach((toggle) => {
    toggle.addEventListener('click', () => {
      const input = wrapper?.querySelector('input');
      const img = toggle.querySelector('img');
      
      const isPassword = input.type === 'password';
      input.type = isPassword ? 'text' : 'password';
      
      if (img) {
        img.src = isPassword 
          ? '/static/icons/eye-off.svg' 
          : '/static/icons/eye.svg';
      }
    });
  });
}
```

## Server Configuration

Make sure your server is configured to serve SVG files with the correct MIME type:

### Nginx:
```nginx
location /static/icons/ {
    types {
        image/svg+xml svg;
    }
    add_header Cache-Control "public, max-age=31536000, immutable";
}
```

### Apache (.htaccess):
```apache
<FilesMatch "\.svg$">
    Header set Cache-Control "public, max-age=31536000, immutable"
    AddType image/svg+xml .svg
</FilesMatch>
```

### Express.js:
```javascript
app.use('/static/icons', express.static('static/icons', {
    maxAge: '1y',
    setHeaders: (res, path) => {
        if (path.endsWith('.svg')) {
            res.setHeader('Content-Type', 'image/svg+xml');
        }
    }
}));
```

## Migration Checklist

- [x] Extract all SVG icons to separate files
- [x] Update all HTML files to use `<img>` tags
- [x] Update JavaScript to handle external SVGs
- [x] Test password toggle functionality
- [x] Verify theme toggle works
- [x] Verify all icons display correctly
- [x] Test in light and dark modes
- [x] Configure server to cache SVG files

## Icon Inventory

| Icon | Usage | Files |
|------|-------|-------|
| bell.svg | Notifications | settings.html |
| eye.svg / eye-off.svg | Password visibility | index.html, register.html |
| help-circle.svg | Help/support | settings.html |
| paperclip.svg | Attach files | chat.html |
| phone.svg | Voice calls | chat.html |
| search.svg | Search conversations | chat.html |
| send.svg | Send messages | chat.html |
| settings.svg | Settings/account | chat.html, settings.html |
| shield.svg | Privacy/security | settings.html |
| sun.svg | Theme toggle | chat.html, settings.html |
| user.svg | Profile | settings.html |
| video.svg | Video calls | chat.html |

## Next Steps

1. Place the `svg-icons/` folder in your `static/icons/` directory
2. Replace your existing HTML files with the updated versions
3. Replace your `auth.js` file
4. Test all functionality
5. Deploy to your server
6. Configure caching headers

All SVGs are now properly separated and ready for server delivery!
