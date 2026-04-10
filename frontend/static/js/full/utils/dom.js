/**
 * DOM Utilities
 * Common helpers for element creation and standard UI patterns.
 */
export const DOM = {
    /**
     * Create an element with attributes and text
     * @param {string} tag - Tag name
     * @param {object} attrs - Attributes to set
     * @param {string|HTMLElement|Array} children - Children to append
     * @returns {HTMLElement} The created element
     */
    create(tag, attrs = {}, children = []) {
        const el = document.createElement(tag);

        Object.entries(attrs).forEach(([key, value]) => {
            if (key === "className") {
                el.className = value;
            } else if (key === "dataset") {
                Object.assign(el.dataset, value);
            } else if (key === "style" && typeof value === "object") {
                Object.assign(el.style, value);
            } else if (key.startsWith("on") && typeof value === "function") {
                el.addEventListener(key.slice(2).toLowerCase(), value);
            } else if (value !== null && value !== undefined) {
                el.setAttribute(key, value);
            }
        });

        const append = (child) => {
            if (typeof child === "string" || typeof child === "number") {
                el.appendChild(document.createTextNode(child));
            } else if (child instanceof HTMLElement) {
                el.appendChild(child);
            }
        };

        if (Array.isArray(children)) {
            children.forEach(append);
        } else if (children) {
            append(children);
        }

        return el;
    },

    /**
     * Clear an element's children and optionally append new ones
     * @param {HTMLElement} el - Element to clear
     * @param {HTMLElement|Array} newChildren - Optional children to append
     */
    clear(el, newChildren = []) {
        if (!el) return;
        while (el.firstChild) el.removeChild(el.firstChild);
        if (newChildren) {
            if (Array.isArray(newChildren)) {
                newChildren.forEach((child) => el.appendChild(child));
            } else {
                el.appendChild(newChildren);
            }
        }
    },

    /**
     * Set focus to an element, optionally after a delay
     * @param {HTMLElement|string} el - Element or id
     * @param {number} delay - Delay in ms
     */
    focus(el, delay = 0) {
        const target = typeof el === "string" ? document.getElementById(el) : el;
        if (!target) return;
        if (delay) {
            setTimeout(() => target.focus(), delay);
        } else {
            target.focus();
        }
    },

    /**
     * Manage focus trap within a container (for accessibility)
     * @param {HTMLElement} container - Container element
     * @returns {Function} Function to release the trap
     */
    trapFocus(container) {
        const focusableElements =
            'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])';
        const focusableContent = container.querySelectorAll(focusableElements);
        const firstFocusable = focusableContent[0];
        const lastFocusable = focusableContent[focusableContent.length - 1];

        const handleKeydown = (e) => {
            if (e.key !== "Tab") return;

            if (e.shiftKey) {
                if (document.activeElement === firstFocusable) {
                    lastFocusable.focus();
                    e.preventDefault();
                }
            } else {
                if (document.activeElement === lastFocusable) {
                    firstFocusable.focus();
                    e.preventDefault();
                }
            }
        };

        container.addEventListener("keydown", handleKeydown);
        return () => container.removeEventListener("keydown", handleKeydown);
    },
};

export default DOM;
