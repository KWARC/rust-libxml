/*
 * This is a small extension to the libxml API.
 * The reason is that we don't convert the structs properly,
 * which prevents some very basic actions
 */
#include <libxml/tree.h>


//returns cur->next
xmlNodePtr xmlNextSibling(xmlNodePtr cur);

//returns cur->prev
xmlNodePtr xmlPrevSibling(xmlNodePtr cur);
